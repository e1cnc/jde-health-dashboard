use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use gloo_timers::callback::Interval;
use futures::stream::{FuturesUnordered, StreamExt};
use urlencoding::encode;
use wasm_bindgen::JsValue;
use web_sys::console;
use std::collections::BTreeMap;

const REFRESH_SECONDS: i32 = 60;
const TICK_MS: u32 = 1000;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    pub instance_status: Option<String>,
    pub health_status: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EnvStatus {
    pub customer: String,
    pub env_name: String,
    pub total: usize,
    pub ok: usize,
    pub err: usize,
    pub filename: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CustomerGroup {
    pub customer: String,
    pub total: usize,
    pub ok: usize,
    pub err: usize,
    pub envs: Vec<EnvStatus>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OCIObject {
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct OCIListResponse {
    #[serde(default)]
    pub objects: Vec<OCIObject>,
    #[serde(default)]
    pub data: Vec<OCIObject>,
}

#[derive(Clone, Copy, PartialEq)]
enum Filter {
    All,
    Failed,
    Healthy,
}

const BASE_URL: &str = "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o";

fn log(msg: &str) {
    console::log_1(&JsValue::from_str(msg));
}

fn parse_customer_env(filename: &str) -> (String, String) {
    let base_name = filename.strip_suffix(".json").unwrap_or(filename);
    let parts: Vec<&str> = base_name.split('_').collect();

    let customer = parts.get(0).unwrap_or(&"UNKNOWN").to_string();
    let env = parts.get(1).unwrap_or(&"UNKNOWN").to_uppercase();

    (customer, env)
}

fn group_by_customer(items: Vec<EnvStatus>, filter: Filter) -> Vec<CustomerGroup> {
    let mut grouped: BTreeMap<String, Vec<EnvStatus>> = BTreeMap::new();

    for item in items.into_iter() {
        let include = match filter {
            Filter::All => true,
            Filter::Failed => item.err > 0,
            Filter::Healthy => item.err == 0,
        };

        if include {
            grouped.entry(item.customer.clone()).or_default().push(item);
        }
    }

    let mut result = Vec::new();

    for (customer, mut envs) in grouped {
        envs.sort_by(|a, b| a.env_name.cmp(&b.env_name));

        let total: usize = envs.iter().map(|e| e.total).sum();
        let ok: usize = envs.iter().map(|e| e.ok).sum();
        let err: usize = envs.iter().map(|e| e.err).sum();

        result.push(CustomerGroup {
            customer,
            total,
            ok,
            err,
            envs,
        });
    }

    result
}

async fn fetch_json_file(filename: &str) -> Result<Vec<HealthInstance>, String> {
    let encoded_name = encode(filename);
    let file_url = format!("{}/{}", BASE_URL, encoded_name);

    log(&format!("Fetching file: {}", file_url));

    let res = Request::get(&file_url)
        .send()
        .await
        .map_err(|e| format!("Fetch error for {}: {}", filename, e))?;

    if !res.ok() {
        return Err(format!("HTTP {} for {}", res.status(), filename));
    }

    res.json::<Vec<HealthInstance>>()
        .await
        .map_err(|e| format!("JSON parse error for {}: {}", filename, e))
}

async fn fetch_jde_health_data() -> Result<Vec<EnvStatus>, String> {
    let list_url = format!("{}/?format=json", BASE_URL);
    log(&format!("Listing URL: {}", list_url));

    let resp = Request::get(&list_url)
        .send()
        .await
        .map_err(|e| format!("List API failed: {}", e))?;

    if !resp.ok() {
        return Err(format!("List API returned HTTP {}", resp.status()));
    }

    let list_data: OCIListResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse OCI list response: {}", e))?;

    let all_objects = if !list_data.objects.is_empty() {
        list_data.objects
    } else {
        list_data.data
    };

    log(&format!("Objects found: {}", all_objects.len()));

    let target_files: Vec<String> = all_objects
        .into_iter()
        .map(|obj| obj.name)
        .filter(|name| name.to_lowercase().ends_with("_latest.json"))
        .collect();

    log(&format!("Matched latest files: {:?}", target_files));

    if target_files.is_empty() {
        return Err("No '_latest.json' files found in bucket listing.".to_string());
    }

    let mut fetch_tasks = FuturesUnordered::new();

    for filename in target_files {
        fetch_tasks.push(async move {
            let instances = fetch_json_file(&filename).await?;
            Ok::<(String, Vec<HealthInstance>), String>((filename, instances))
        });
    }

    let mut results = Vec::new();
    let mut errors = Vec::new();

    while let Some(task_result) = fetch_tasks.next().await {
        match task_result {
            Ok((filename, instances)) => {
                let (customer, env_name) = parse_customer_env(&filename);

                let total = instances.len();
                let mut ok = 0usize;
                let mut err = 0usize;

                for inst in &instances {
                    let instance_status = inst
                        .instance_status
                        .as_deref()
                        .unwrap_or("")
                        .trim()
                        .to_uppercase();

                    let health_status = inst
                        .health_status
                        .as_deref()
                        .unwrap_or("")
                        .trim()
                        .to_lowercase();

                    if instance_status == "RUNNING" && health_status == "passed" {
                        ok += 1;
                    } else {
                        err += 1;
                    }
                }

                results.push(EnvStatus {
                    customer,
                    env_name,
                    total,
                    ok,
                    err,
                    filename,
                });
            }
            Err(e) => {
                log(&e);
                errors.push(e);
            }
        }
    }

    if results.is_empty() {
        return Err(format!(
            "Could not load any environment data. Errors: {}",
            errors.join(" | ")
        ));
    }

    results.sort_by(|a, b| {
        a.customer
            .cmp(&b.customer)
            .then(a.env_name.cmp(&b.env_name))
    });

    Ok(results)
}

#[component]
fn App() -> impl IntoView {
    let (filter, set_filter) = create_signal(Filter::All);
    let (seconds_left, set_seconds_left) = create_signal(REFRESH_SECONDS);
    let (selected_env, set_selected_env) = create_signal::<Option<EnvStatus>>(None);

    let health_resource = create_resource(
        || (),
        |_| async move { fetch_jde_health_data().await },
    );

    let detail_resource = create_resource(
        move || selected_env.get(),
        |selected| async move {
            match selected {
                Some(env) => {
                    let raw_json = fetch_json_file(&env.filename).await?;
                    let pretty = serde_json::to_string_pretty(&raw_json)
                        .map_err(|e| format!("Failed to format JSON: {}", e))?;
                    Ok::<(EnvStatus, String), String>((env, pretty))
                }
                None => Err("No environment selected.".to_string()),
            }
        },
    );

    {
        let health_resource = health_resource;
        let detail_resource = detail_resource;

        create_effect(move |_| {
            let interval = Interval::new(TICK_MS, move || {
                let current = seconds_left.get_untracked();

                if current <= 1 {
                    set_seconds_left.set(REFRESH_SECONDS);
                    health_resource.refetch();

                    if selected_env.get_untracked().is_some() {
                        detail_resource.refetch();
                    }
                } else {
                    set_seconds_left.set(current - 1);
                }
            });

            on_cleanup(move || drop(interval));
        });
    }

    let refresh_pct = move || {
        let elapsed = REFRESH_SECONDS - seconds_left.get();
        (elapsed as f32 / REFRESH_SECONDS as f32) * 100.0
    };

    view! {
        <div style="padding: 25px; background: #f8fafc; min-height: 100vh; font-family: Arial, sans-serif;">
            <div style="max-width: 1380px; margin: auto;">
                <Show
                    when=move || selected_env.get().is_none()
                    fallback=move || {
                        view! {
                            <Transition fallback=|| view! { <p>"Loading detail..."</p> }>
                                {move || detail_resource.get().map(|res| match res {
                                    Err(e) => view! {
                                        <>
                                            <button
                                                on:click=move |_| set_selected_env.set(None)
                                                style="margin-bottom: 16px; border: none; background: #1e293b; color: white; padding: 10px 16px; border-radius: 8px; cursor: pointer; font-weight: 700;"
                                            >
                                                "← Back to dashboard"
                                            </button>

                                            <div style="background: white; border-radius: 12px; padding: 20px; color: #dc2626; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                {e}
                                            </div>
                                        </>
                                    }.into_view(),

                                    Ok((env, pretty_json)) => {
                                        let pct = if env.total > 0 {
                                            (env.ok as f32 / env.total as f32) * 100.0
                                        } else {
                                            0.0
                                        };

                                        view! {
                                            <>
                                                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 18px; gap: 12px; flex-wrap: wrap;">
                                                    <button
                                                        on:click=move |_| set_selected_env.set(None)
                                                        style="border: none; background: #1e293b; color: white; padding: 10px 16px; border-radius: 8px; cursor: pointer; font-weight: 700;"
                                                    >
                                                        "← Back to dashboard"
                                                    </button>

                                                    <button
                                                        on:click=move |_| detail_resource.refetch()
                                                        style="border: none; background: #2563eb; color: white; padding: 10px 16px; border-radius: 8px; cursor: pointer; font-weight: 700;"
                                                    >
                                                        "Refresh selected env"
                                                    </button>
                                                </div>

                                                <div style="background: white; border-radius: 12px; padding: 20px; margin-bottom: 20px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                    <div style="color: #94a3b8; font-size: 0.75rem; font-weight: 800; text-transform: uppercase;">
                                                        {env.customer.clone()}
                                                    </div>

                                                    <div style="color: #0f172a; font-size: 1.8rem; font-weight: 900; margin: 6px 0 14px 0;">
                                                        {env.env_name.clone()}
                                                    </div>

                                                    <div style="display: flex; gap: 18px; flex-wrap: wrap; color: #475569; font-size: 0.95rem; margin-bottom: 16px;">
                                                        <div>{format!("Total: {}", env.total)}</div>
                                                        <div>{format!("OK: {}", env.ok)}</div>
                                                        <div>{format!("Error: {}", env.err)}</div>
                                                        <div>{format!("Health: {:.1}%", pct)}</div>
                                                        <div>{format!("Source: {}", env.filename)}</div>
                                                    </div>

                                                    <div style="background: #e2e8f0; height: 8px; border-radius: 999px; overflow: hidden;">
                                                        <div style=format!(
                                                            "height: 100%; width: {:.2}%; background: {}; transition: width 0.4s;",
                                                            pct,
                                                            if env.err == 0 { "#10b981" } else { "#ef4444" }
                                                        )></div>
                                                    </div>
                                                </div>

                                                <div style="background: #0f172a; color: #e2e8f0; border-radius: 12px; padding: 20px; box-shadow: 0 6px 18px rgba(0,0,0,0.12);">
                                                    <div style="font-weight: 800; margin-bottom: 12px; color: #f8fafc;">
                                                        "Raw JSON"
                                                    </div>
                                                    <pre style="margin: 0; white-space: pre-wrap; word-break: break-word; font-size: 0.85rem; line-height: 1.5; overflow-x: auto;">
                                                        {pretty_json}
                                                    </pre>
                                                </div>
                                            </>
                                        }.into_view()
                                    }
                                })}
                            </Transition>
                        }
                    }
                >
                    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px; gap: 12px; flex-wrap: wrap;">
                        <h2 style="margin: 0; color: #0f172a; font-weight: 900; letter-spacing: 0.3px;">
                            "JDE GLOBAL MONITOR"
                        </h2>

                        <div style="display: flex; align-items: center; gap: 12px; flex-wrap: wrap;">
                            <div style="min-width: 220px;">
                                <div style="display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 0.8rem; color: #64748b; font-weight: 700;">
                                    <span>"Auto refresh"</span>
                                    <span>{move || format!("{}s", seconds_left.get())}</span>
                                </div>

                                <div style="background: #e2e8f0; height: 8px; border-radius: 999px; overflow: hidden;">
                                    <div style=move || format!(
                                        "height: 100%; width: {:.2}%; background: #2563eb; transition: width 1s linear;",
                                        refresh_pct()
                                    )></div>
                                </div>
                            </div>

                            <button
                                on:click=move |_| {
                                    set_seconds_left.set(REFRESH_SECONDS);
                                    health_resource.refetch();
                                }
                                style="border: none; background: #2563eb; color: white; padding: 10px 14px; border-radius: 8px; cursor: pointer; font-weight: 700;"
                            >
                                "Refresh now"
                            </button>
                        </div>
                    </div>

                    <div style="display: flex; gap: 5px; background: #f1f5f9; padding: 4px; border-radius: 8px; width: fit-content; margin-bottom: 24px;">
                        <button
                            on:click=move |_| set_filter.set(Filter::All)
                            style=move || format!(
                                "border: none; padding: 8px 16px; border-radius: 6px; cursor: pointer; font-weight: 700; background: {}; color: {};",
                                if filter.get() == Filter::All { "#1e293b" } else { "transparent" },
                                if filter.get() == Filter::All { "white" } else { "#64748b" }
                            )
                        >
                            "ALL"
                        </button>

                        <button
                            on:click=move |_| set_filter.set(Filter::Failed)
                            style=move || format!(
                                "border: none; padding: 8px 16px; border-radius: 6px; cursor: pointer; font-weight: 700; background: {}; color: {};",
                                if filter.get() == Filter::Failed { "#ef4444" } else { "transparent" },
                                if filter.get() == Filter::Failed { "white" } else { "#64748b" }
                            )
                        >
                            "FAILED"
                        </button>

                        <button
                            on:click=move |_| set_filter.set(Filter::Healthy)
                            style=move || format!(
                                "border: none; padding: 8px 16px; border-radius: 6px; cursor: pointer; font-weight: 700; background: {}; color: {};",
                                if filter.get() == Filter::Healthy { "#10b981" } else { "transparent" },
                                if filter.get() == Filter::Healthy { "white" } else { "#64748b" }
                            )
                        >
                            "HEALTHY"
                        </button>
                    </div>

                    <Transition fallback=|| view! { <p>"Processing..."</p> }>
                        {move || health_resource.get().map(|res| match res {
                            Err(e) => view! {
                                <div style="color: #ef4444; padding: 20px; background: white; border-radius: 8px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                    <div style="font-weight: 700; margin-bottom: 6px;">"Load failed"</div>
                                    <div>{e}</div>
                                </div>
                            }.into_view(),

                            Ok(items) => {
                                let total_ok: usize = items.iter().map(|i| i.ok).sum();
                                let total_inst: usize = items.iter().map(|i| i.total).sum();
                                let total_err: usize = items.iter().map(|i| i.err).sum();

                                let health_pct = if total_inst > 0 {
                                    (total_ok as f32 / total_inst as f32) * 100.0
                                } else {
                                    0.0
                                };

                                let customer_groups = group_by_customer(items, filter.get());

                                view! {
                                    <>
                                        <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 16px; margin-bottom: 24px;">
                                            <div style="background: white; border-radius: 12px; padding: 18px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                <div style="color: #94a3b8; font-size: 0.75rem; font-weight: 800; text-transform: uppercase;">"Total Instances"</div>
                                                <div style="font-size: 1.8rem; font-weight: 900; color: #0f172a; margin-top: 8px;">{total_inst}</div>
                                            </div>

                                            <div style="background: white; border-radius: 12px; padding: 18px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                <div style="color: #94a3b8; font-size: 0.75rem; font-weight: 800; text-transform: uppercase;">"Healthy"</div>
                                                <div style="font-size: 1.8rem; font-weight: 900; color: #10b981; margin-top: 8px;">{total_ok}</div>
                                            </div>

                                            <div style="background: white; border-radius: 12px; padding: 18px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                <div style="color: #94a3b8; font-size: 0.75rem; font-weight: 800; text-transform: uppercase;">"Errors"</div>
                                                <div style="font-size: 1.8rem; font-weight: 900; color: #ef4444; margin-top: 8px;">{total_err}</div>
                                            </div>

                                            <div style="background: white; border-radius: 12px; padding: 18px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                <div style="color: #94a3b8; font-size: 0.75rem; font-weight: 800; text-transform: uppercase;">"Overall Health"</div>
                                                <div style="font-size: 1.8rem; font-weight: 900; color: #2563eb; margin-top: 8px;">{format!("{:.1}%", health_pct)}</div>
                                            </div>
                                        </div>

                                        <div style="background: white; border-radius: 12px; padding: 20px; margin-bottom: 24px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px;">
                                                <span style="font-weight: 800; color: #1e293b;">"SYSTEM WIDE HEALTH"</span>
                                                <span style=format!(
                                                    "font-weight: 900; color: {};",
                                                    if health_pct > 90.0 { "#10b981" } else { "#ef4444" }
                                                )>
                                                    {format!("{:.1}%", health_pct)}
                                                </span>
                                            </div>

                                            <div style="background: #f1f5f9; height: 10px; border-radius: 999px; overflow: hidden;">
                                                <div style=format!(
                                                    "background: {}; height: 100%; width: {:.2}%; transition: width 0.4s;",
                                                    if health_pct > 90.0 { "#10b981" } else { "#ef4444" },
                                                    health_pct
                                                )></div>
                                            </div>

                                            <div style="margin-top: 10px; font-size: 0.85rem; color: #64748b;">
                                                {format!("{} healthy out of {} instances", total_ok, total_inst)}
                                            </div>
                                        </div>

                                        <div style="display: grid; gap: 24px;">
                                            {
                                                customer_groups
                                                    .into_iter()
                                                    .map(|group| {
                                                        let customer_pct = if group.total > 0 {
                                                            (group.ok as f32 / group.total as f32) * 100.0
                                                        } else {
                                                            0.0
                                                        };

                                                        let customer_healthy = group.err == 0;

                                                        view! {
                                                            <div style="background: #ffffff; border-radius: 16px; padding: 22px; box-shadow: 0 2px 8px rgba(0,0,0,0.08);">
                                                                <div style="display: flex; justify-content: space-between; align-items: flex-start; gap: 16px; flex-wrap: wrap; margin-bottom: 18px;">
                                                                    <div>
                                                                        <div style="color: #94a3b8; font-size: 0.75rem; font-weight: 800; text-transform: uppercase; margin-bottom: 6px;">
                                                                            "Customer"
                                                                        </div>
                                                                        <div style="font-size: 1.5rem; font-weight: 900; color: #0f172a;">
                                                                            {group.customer.clone()}
                                                                        </div>
                                                                    </div>

                                                                    <div style="min-width: 260px; flex: 1; max-width: 420px;">
                                                                        <div style="display: flex; justify-content: space-between; font-size: 0.85rem; margin-bottom: 8px;">
                                                                            <span style="color: #475569; font-weight: 700;">
                                                                                {if customer_healthy { "Group Healthy" } else { "Group Errors Present" }}
                                                                            </span>
                                                                            <span style=format!(
                                                                                "font-weight: 900; color: {};",
                                                                                if customer_healthy { "#10b981" } else { "#ef4444" }
                                                                            )>
                                                                                {format!("{:.1}%", customer_pct)}
                                                                            </span>
                                                                        </div>

                                                                        <div style="background: #e2e8f0; height: 9px; border-radius: 999px; overflow: hidden;">
                                                                            <div style=format!(
                                                                                "background: {}; height: 100%; width: {:.2}%; transition: width 0.4s;",
                                                                                if customer_healthy { "#10b981" } else { "#ef4444" },
                                                                                customer_pct
                                                                            )></div>
                                                                        </div>

                                                                        <div style="display: flex; gap: 14px; flex-wrap: wrap; margin-top: 10px; font-size: 0.85rem; color: #64748b;">
                                                                            <span>{format!("Envs: {}", group.envs.len())}</span>
                                                                            <span>{format!("Total: {}", group.total)}</span>
                                                                            <span>{format!("OK: {}", group.ok)}</span>
                                                                            <span>{format!("Error: {}", group.err)}</span>
                                                                        </div>
                                                                    </div>
                                                                </div>

                                                                <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 18px;">
                                                                    {
                                                                        group.envs
                                                                            .into_iter()
                                                                            .map(|item| {
                                                                                let is_healthy = item.err == 0;
                                                                                let pct = if item.total > 0 {
                                                                                    (item.ok as f32 / item.total as f32) * 100.0
                                                                                } else {
                                                                                    0.0
                                                                                };

                                                                                let item_for_click = item.clone();

                                                                                view! {
                                                                                    <div
                                                                                        on:click=move |_| set_selected_env.set(Some(item_for_click.clone()))
                                                                                        style=format!(
                                                                                            "background: #fff; border-radius: 12px; padding: 20px; box-shadow: 0 4px 6px -1px rgba(0,0,0,0.08); border-top: 4px solid {}; cursor: pointer;",
                                                                                            if is_healthy { "#10b981" } else { "#ef4444" }
                                                                                        )
                                                                                    >
                                                                                        <div style="color: #94a3b8; font-size: 0.7rem; font-weight: 800; text-transform: uppercase;">
                                                                                            {item.customer.clone()}
                                                                                        </div>

                                                                                        <div style="color: #1e293b; font-size: 1.4rem; font-weight: 900; margin-bottom: 15px;">
                                                                                            {item.env_name.clone()}
                                                                                        </div>

                                                                                        <div style="display: grid; gap: 6px; margin-bottom: 14px; font-size: 0.85rem; color: #475569;">
                                                                                            <div>{format!("Total: {}", item.total)}</div>
                                                                                            <div>{format!("OK: {}", item.ok)}</div>
                                                                                            <div>{format!("Error: {}", item.err)}</div>
                                                                                        </div>

                                                                                        <div style="display: flex; justify-content: space-between; align-items: center; border-top: 1px solid #f1f5f9; padding-top: 15px;">
                                                                                            <div>
                                                                                                <div style=format!(
                                                                                                    "font-weight: 800; font-size: 0.8rem; color: {};",
                                                                                                    if is_healthy { "#059669" } else { "#dc2626" }
                                                                                                )>
                                                                                                    {if is_healthy { "HEALTHY" } else { "ERROR" }}
                                                                                                </div>

                                                                                                <div style="font-size: 0.75rem; color: #64748b;">
                                                                                                    {format!("{}/{} OK", item.ok, item.total)}
                                                                                                </div>
                                                                                            </div>

                                                                                            <div style=format!(
                                                                                                "font-size: 1.6rem; font-weight: 900; color: {};",
                                                                                                if is_healthy { "#10b981" } else { "#ef4444" }
                                                                                            )>
                                                                                                {format!("{:.0}%", pct)}
                                                                                            </div>
                                                                                        </div>
                                                                                    </div>
                                                                                }
                                                                            })
                                                                            .collect_view()
                                                                    }
                                                                </div>
                                                            </div>
                                                        }
                                                    })
                                                    .collect_view()
                                            }
                                        </div>
                                    </>
                                }.into_view()
                            }
                        })}
                    </Transition>
                </Show>
            </div>
        </div>
    }
}

fn main() {
    mount_to_body(|| view! { <App /> })
}