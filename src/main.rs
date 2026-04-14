use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use gloo_timers::callback::Interval;
use futures::stream::{FuturesUnordered, StreamExt};
use urlencoding::encode;
use wasm_bindgen::{JsCast, JsValue};
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomerChartDatum {
    pub customer: String,
    pub total: usize,
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

fn calc_pct(ok: usize, total: usize) -> f32 {
    if total > 0 {
        (ok as f32 / total as f32) * 100.0
    } else {
        0.0
    }
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

fn build_customer_chart_data(groups: &[CustomerGroup]) -> Vec<CustomerChartDatum> {
    let mut data: Vec<CustomerChartDatum> = groups
        .iter()
        .map(|g| CustomerChartDatum {
            customer: g.customer.clone(),
            total: g.total,
        })
        .collect();

    data.sort_by(|a, b| b.total.cmp(&a.total).then(a.customer.cmp(&b.customer)));
    data
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
fn DoughnutChart(data: Vec<CustomerChartDatum>) -> impl IntoView {
    let canvas_ref = create_node_ref::<html::Canvas>();

    create_effect(move |_| {
        let Some(canvas) = canvas_ref.get() else {
            return;
        };

        let labels = data.iter().map(|d| d.customer.clone()).collect::<Vec<_>>();
        let values = data.iter().map(|d| d.total as f64).collect::<Vec<_>>();
        let colors = vec![
            "#0ea5e9", "#f97316", "#22c55e", "#ef4444", "#8b5cf6", "#eab308",
            "#14b8a6", "#ec4899", "#6366f1", "#84cc16", "#06b6d4", "#f59e0b",
            "#10b981", "#f43f5e", "#a855f7", "#3b82f6", "#78716c", "#64748b",
        ];

        let labels_js = serde_wasm_bindgen::to_value(&labels).unwrap_or(JsValue::NULL);
        let values_js = serde_wasm_bindgen::to_value(&values).unwrap_or(JsValue::NULL);
        let colors_js = serde_wasm_bindgen::to_value(&colors).unwrap_or(JsValue::NULL);

        let chart_ctor = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("Chart"))
            .ok()
            .filter(|v| !v.is_undefined() && !v.is_null());

        let Some(chart_ctor) = chart_ctor else {
            log("Chart.js is not loaded on window.Chart");
            return;
        };

        let window = web_sys::window().unwrap();
        let chart_key = JsValue::from_str("__jde_customer_chart");

        if let Ok(existing) = js_sys::Reflect::get(&window, &chart_key) {
            if !existing.is_undefined() && !existing.is_null() {
                if let Ok(destroy_fn) =
                    js_sys::Reflect::get(&existing, &JsValue::from_str("destroy"))
                {
                    if let Some(destroy) = destroy_fn.dyn_ref::<js_sys::Function>() {
                        let _ = destroy.call0(&existing);
                    }
                }
            }
        }

        let data_obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&data_obj, &JsValue::from_str("labels"), &labels_js);

        let dataset = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&dataset, &JsValue::from_str("data"), &values_js);
        let _ = js_sys::Reflect::set(&dataset, &JsValue::from_str("backgroundColor"), &colors_js);
        let _ = js_sys::Reflect::set(&dataset, &JsValue::from_str("borderColor"), &JsValue::from_str("#ffffff"));
        let _ = js_sys::Reflect::set(&dataset, &JsValue::from_str("borderWidth"), &JsValue::from_f64(2.0));
        let _ = js_sys::Reflect::set(&dataset, &JsValue::from_str("hoverOffset"), &JsValue::from_f64(8.0));

        let datasets = js_sys::Array::new();
        datasets.push(&dataset);
        let _ = js_sys::Reflect::set(&data_obj, &JsValue::from_str("datasets"), &datasets.into());

        let font_obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&font_obj, &JsValue::from_str("size"), &JsValue::from_f64(11.0));
        let _ = js_sys::Reflect::set(&font_obj, &JsValue::from_str("weight"), &JsValue::from_str("600"));

        let legend_labels = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&legend_labels, &JsValue::from_str("usePointStyle"), &JsValue::TRUE);
        let _ = js_sys::Reflect::set(&legend_labels, &JsValue::from_str("pointStyle"), &JsValue::from_str("circle"));
        let _ = js_sys::Reflect::set(&legend_labels, &JsValue::from_str("boxWidth"), &JsValue::from_f64(10.0));
        let _ = js_sys::Reflect::set(&legend_labels, &JsValue::from_str("boxHeight"), &JsValue::from_f64(10.0));
        let _ = js_sys::Reflect::set(&legend_labels, &JsValue::from_str("padding"), &JsValue::from_f64(14.0));
        let _ = js_sys::Reflect::set(&legend_labels, &JsValue::from_str("color"), &JsValue::from_str("#334155"));
        let _ = js_sys::Reflect::set(&legend_labels, &JsValue::from_str("font"), &font_obj);

        let legend_obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&legend_obj, &JsValue::from_str("position"), &JsValue::from_str("right"));
        let _ = js_sys::Reflect::set(&legend_obj, &JsValue::from_str("labels"), &legend_labels);

        let plugins_obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&plugins_obj, &JsValue::from_str("legend"), &legend_obj);

        let options_obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&options_obj, &JsValue::from_str("responsive"), &JsValue::TRUE);
        let _ = js_sys::Reflect::set(&options_obj, &JsValue::from_str("maintainAspectRatio"), &JsValue::FALSE);
        let _ = js_sys::Reflect::set(&options_obj, &JsValue::from_str("cutout"), &JsValue::from_str("65%"));
        let _ = js_sys::Reflect::set(&options_obj, &JsValue::from_str("plugins"), &plugins_obj);

        let config = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&config, &JsValue::from_str("type"), &JsValue::from_str("doughnut"));
        let _ = js_sys::Reflect::set(&config, &JsValue::from_str("data"), &data_obj);
        let _ = js_sys::Reflect::set(&config, &JsValue::from_str("options"), &options_obj);

        let args = js_sys::Array::new();
        args.push(canvas.as_ref());
        args.push(&config);

        if let Some(constructor) = chart_ctor.dyn_ref::<js_sys::Function>() {
            if let Ok(chart_instance) = js_sys::Reflect::construct(constructor, &args) {
                let _ = js_sys::Reflect::set(&window, &chart_key, &chart_instance);
            } else {
                log("Failed to construct Chart.js chart");
            }
        } else {
            log("window.Chart is not callable");
        }
    });

    view! {
        <div style="height: 180px; max-width: 260px; margin: 0 auto; position: relative;">
            <canvas node_ref=canvas_ref style="width: 100%; height: 100%;"></canvas>
        </div>
    }
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
        <>
            <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>

            <div style="padding: 14px; background: #f8fafc; min-height: 100vh; font-family: Arial, sans-serif;">
                <div style="max-width: 1800px; margin: auto;">
                    <Show
                        when=move || selected_env.get().is_none()
                        fallback=move || {
                            view! {
                                <Transition fallback=|| view! { <p>"Loading detail..."</p> }>
                                    {move || {
                                        detail_resource.get().map(|res| match res {
                                            Err(e) => view! {
                                                <>
                                                    <button
                                                        on:click=move |_| set_selected_env.set(None)
                                                        style="margin-bottom: 12px; border: none; background: #1e293b; color: white; padding: 9px 14px; border-radius: 8px; cursor: pointer; font-weight: 700;"
                                                    >
                                                        "← Back to dashboard"
                                                    </button>

                                                    <div style="background: white; border-radius: 12px; padding: 16px; color: #dc2626; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                        {e}
                                                    </div>
                                                </>
                                            }.into_view(),

                                            Ok((env, pretty_json)) => {
                                                let pct = calc_pct(env.ok, env.total);

                                                view! {
                                                    <>
                                                        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; gap: 10px; flex-wrap: wrap;">
                                                            <button
                                                                on:click=move |_| set_selected_env.set(None)
                                                                style="border: none; background: #1e293b; color: white; padding: 9px 14px; border-radius: 8px; cursor: pointer; font-weight: 700;"
                                                            >
                                                                "← Back to dashboard"
                                                            </button>

                                                            <button
                                                                on:click=move |_| detail_resource.refetch()
                                                                style="border: none; background: #2563eb; color: white; padding: 9px 14px; border-radius: 8px; cursor: pointer; font-weight: 700;"
                                                            >
                                                                "Refresh selected env"
                                                            </button>
                                                        </div>

                                                        <div style="background: white; border-radius: 12px; padding: 16px; margin-bottom: 14px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                            <div style="color: #94a3b8; font-size: 0.68rem; font-weight: 800; text-transform: uppercase;">
                                                                {env.customer.clone()}
                                                            </div>

                                                            <div style="color: #0f172a; font-size: 1.35rem; font-weight: 900; margin: 6px 0 10px 0;">
                                                                {env.env_name.clone()}
                                                            </div>

                                                            <div style="display: flex; gap: 12px; flex-wrap: wrap; color: #475569; font-size: 0.82rem; margin-bottom: 12px;">
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

                                                        <div style="background: #0f172a; color: #e2e8f0; border-radius: 12px; padding: 16px; box-shadow: 0 6px 18px rgba(0,0,0,0.12);">
                                                            <div style="font-weight: 800; margin-bottom: 10px; color: #f8fafc;">
                                                                "Raw JSON"
                                                            </div>
                                                            <pre style="margin: 0; white-space: pre-wrap; word-break: break-word; font-size: 0.78rem; line-height: 1.42; overflow-x: auto;">
                                                                {pretty_json}
                                                            </pre>
                                                        </div>
                                                    </>
                                                }.into_view()
                                            }
                                        })
                                    }}
                                </Transition>
                            }
                        }
                    >
                        <div
                            style="
                                display: grid;
                                grid-template-columns: auto 1fr auto;
                                align-items: center;
                                gap: 16px;
                                margin-bottom: 14px;
                                width: 100%;
                            "
                        >
                            <div style="display: flex; justify-content: flex-start;">
                                <div
                                    style="
                                        display: flex;
                                        gap: 4px;
                                        background: #e2e8f0;
                                        padding: 4px;
                                        border-radius: 10px;
                                        width: fit-content;
                                    "
                                >
                                    <button
                                        on:click=move |_| set_filter.set(Filter::All)
                                        style=move || format!(
                                            "border: none; padding: 10px 18px; border-radius: 8px; cursor: pointer; font-weight: 800; font-size: 0.85rem; background: {}; color: {}; white-space: nowrap;",
                                            if filter.get() == Filter::All { "#1e293b" } else { "transparent" },
                                            if filter.get() == Filter::All { "white" } else { "#64748b" }
                                        )
                                    >
                                        "ALL"
                                    </button>

                                    <button
                                        on:click=move |_| set_filter.set(Filter::Failed)
                                        style=move || format!(
                                            "border: none; padding: 10px 18px; border-radius: 8px; cursor: pointer; font-weight: 800; font-size: 0.85rem; background: {}; color: {}; white-space: nowrap;",
                                            if filter.get() == Filter::Failed { "#ef4444" } else { "transparent" },
                                            if filter.get() == Filter::Failed { "white" } else { "#64748b" }
                                        )
                                    >
                                        "FAILED"
                                    </button>

                                    <button
                                        on:click=move |_| set_filter.set(Filter::Healthy)
                                        style=move || format!(
                                            "border: none; padding: 10px 18px; border-radius: 8px; cursor: pointer; font-weight: 800; font-size: 0.85rem; background: {}; color: {}; white-space: nowrap;",
                                            if filter.get() == Filter::Healthy { "#10b981" } else { "transparent" },
                                            if filter.get() == Filter::Healthy { "white" } else { "#64748b" }
                                        )
                                    >
                                        "HEALTHY"
                                    </button>
                                </div>
                            </div>

                            <div style="display: flex; justify-content: center; min-width: 0;">
                                <h2
                                    style="
                                        margin: 0;
                                        color: #1d4ed8;
                                        font-weight: 900;
                                        letter-spacing: 0.3px;
                                        font-size: 1.15rem;
                                        text-align: center;
                                        white-space: nowrap;
                                    "
                                >
                                    "JDE Environment Health Dashboard"
                                </h2>
                            </div>

                            <div
                                style="
                                    display: flex;
                                    justify-content: flex-end;
                                    align-items: center;
                                    gap: 12px;
                                    white-space: nowrap;
                                "
                            >
                                <div style="width: 330px;">
                                    <div
                                        style="
                                            display: flex;
                                            justify-content: space-between;
                                            margin-bottom: 5px;
                                            font-size: 0.74rem;
                                            color: #64748b;
                                            font-weight: 700;
                                        "
                                    >
                                        <span>"Auto refresh"</span>
                                        <span>{move || format!("{}s", seconds_left.get())}</span>
                                    </div>

                                    <div
                                        style="
                                            background: #cbd5e1;
                                            height: 8px;
                                            border-radius: 999px;
                                            overflow: hidden;
                                        "
                                    >
                                        <div
                                            style=move || format!(
                                                "height: 100%; width: {:.2}%; background: #2563eb; transition: width 1s linear;",
                                                refresh_pct()
                                            )
                                        ></div>
                                    </div>
                                </div>

                                <button
                                    on:click=move |_| {
                                        set_seconds_left.set(REFRESH_SECONDS);
                                        health_resource.refetch();
                                    }
                                    style="
                                        border: none;
                                        background: #2563eb;
                                        color: white;
                                        padding: 12px 18px;
                                        border-radius: 12px;
                                        cursor: pointer;
                                        font-weight: 800;
                                        font-size: 0.9rem;
                                        white-space: nowrap;
                                    "
                                >
                                    "Refresh now"
                                </button>
                            </div>
                        </div>

                        <Transition fallback=|| view! { <p>"Processing..."</p> }>
                            {move || {
                                health_resource.get().map(|res| match res {
                                    Err(e) => view! {
                                        <div style="color: #ef4444; padding: 16px; background: white; border-radius: 8px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                            <div style="font-weight: 700; margin-bottom: 6px;">"Load failed"</div>
                                            <div>{e}</div>
                                        </div>
                                    }.into_view(),

                                    Ok(items) => {
                                        let filtered_for_summary: Vec<EnvStatus> = items
                                            .iter()
                                            .cloned()
                                            .filter(|item| match filter.get() {
                                                Filter::All => true,
                                                Filter::Failed => item.err > 0,
                                                Filter::Healthy => item.err == 0,
                                            })
                                            .collect();

                                        let total_ok: usize = filtered_for_summary.iter().map(|i| i.ok).sum();
                                        let total_inst: usize = filtered_for_summary.iter().map(|i| i.total).sum();
                                        let total_err: usize = filtered_for_summary.iter().map(|i| i.err).sum();

                                        let total_customers: usize = {
                                            let mut s = BTreeMap::new();
                                            for item in &filtered_for_summary {
                                                s.insert(item.customer.clone(), true);
                                            }
                                            s.len()
                                        };

                                        let health_pct = calc_pct(total_ok, total_inst);
                                        let customer_groups = group_by_customer(items, filter.get());
                                        let chart_data = build_customer_chart_data(&customer_groups);

                                        view! {
                                            <>
                                                <div style="display: grid; grid-template-columns: minmax(320px, 420px) 1fr; gap: 12px; margin-bottom: 14px;">
                                                    <div style="background: white; border-radius: 12px; padding: 14px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; gap: 8px; flex-wrap: wrap;">
                                                            <div>
                                                                <div style="color: #94a3b8; font-size: 0.68rem; font-weight: 800; text-transform: uppercase;">
                                                                    "Customer Distribution"
                                                                </div>
                                                                <div style="color: #0f172a; font-size: 0.92rem; font-weight: 900; margin-top: 4px;">
                                                                    "Instances by customer"
                                                                </div>
                                                            </div>
                                                            <div style="font-size: 0.72rem; color: #64748b; font-weight: 700;">
                                                                {format!("{} customers", total_customers)}
                                                            </div>
                                                        </div>

                                                        <DoughnutChart data=chart_data />
                                                    </div>

                                                    <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 10px;">
                                                        <div style="background: white; border-radius: 10px; padding: 12px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                            <div style="color: #94a3b8; font-size: 0.64rem; font-weight: 800; text-transform: uppercase;">"Customers"</div>
                                                            <div style="font-size: 1.25rem; font-weight: 900; color: #0f172a; margin-top: 6px;">{total_customers}</div>
                                                        </div>

                                                        <div style="background: white; border-radius: 10px; padding: 12px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                            <div style="color: #94a3b8; font-size: 0.64rem; font-weight: 800; text-transform: uppercase;">"Instances"</div>
                                                            <div style="font-size: 1.25rem; font-weight: 900; color: #0f172a; margin-top: 6px;">{total_inst}</div>
                                                        </div>

                                                        <div style="background: white; border-radius: 10px; padding: 12px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                            <div style="color: #94a3b8; font-size: 0.64rem; font-weight: 800; text-transform: uppercase;">"Healthy"</div>
                                                            <div style="font-size: 1.25rem; font-weight: 900; color: #10b981; margin-top: 6px;">{total_ok}</div>
                                                        </div>

                                                        <div style="background: white; border-radius: 10px; padding: 12px; box-shadow: 0 1px 3px rgba(0,0,0,0.08);">
                                                            <div style="color: #94a3b8; font-size: 0.64rem; font-weight: 800; text-transform: uppercase;">"Errors"</div>
                                                            <div style="font-size: 1.25rem; font-weight: 900; color: #ef4444; margin-top: 6px;">{total_err}</div>
                                                        </div>

                                                        <div style="background: white; border-radius: 10px; padding: 12px; box-shadow: 0 1px 3px rgba(0,0,0,0.08); grid-column: span 2;">
                                                            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;">
                                                                <span style="font-weight: 800; color: #1e293b; font-size: 0.82rem;">"OVERALL HEALTH"</span>
                                                                <span style=format!(
                                                                    "font-weight: 900; font-size: 0.92rem; color: {};",
                                                                    if health_pct > 90.0 { "#10b981" } else { "#ef4444" }
                                                                )>
                                                                    {format!("{:.1}%", health_pct)}
                                                                </span>
                                                            </div>

                                                            <div style="background: #f1f5f9; height: 8px; border-radius: 999px; overflow: hidden;">
                                                                <div style=format!(
                                                                    "background: {}; height: 100%; width: {:.2}%; transition: width 0.4s;",
                                                                    if health_pct > 90.0 { "#10b981" } else { "#ef4444" },
                                                                    health_pct
                                                                )></div>
                                                            </div>

                                                            <div style="margin-top: 8px; font-size: 0.72rem; color: #64748b;">
                                                                {format!("{} healthy out of {} instances", total_ok, total_inst)}
                                                            </div>
                                                        </div>
                                                    </div>
                                                </div>

                                                <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(440px, 1fr)); gap: 12px; align-items: stretch;">
                                                    {
                                                        customer_groups
                                                            .into_iter()
                                                            .map(|group| {
                                                                let customer_pct = calc_pct(group.ok, group.total);
                                                                let customer_healthy = group.err == 0;
                                                                let env_count = group.envs.len();

                                                                view! {
                                                                    <div style="background: #ffffff; border-radius: 12px; padding: 12px; box-shadow: 0 1px 4px rgba(0,0,0,0.08); min-height: 420px; display: flex; flex-direction: column;">
                                                                        <div>
                                                                            <div style="color: #94a3b8; font-size: 0.58rem; font-weight: 800; text-transform: uppercase; margin-bottom: 2px;">
                                                                                "CUSTOMER"
                                                                            </div>

                                                                            <div style="font-size: 0.98rem; font-weight: 900; color: #0f172a; line-height: 1.15; margin-bottom: 10px;">
                                                                                {group.customer.clone()}
                                                                            </div>

                                                                            <div style="display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px;">
                                                                                {
                                                                                    group.envs
                                                                                        .into_iter()
                                                                                        .map(|item| {
                                                                                            let is_healthy = item.err == 0;
                                                                                            let pct = calc_pct(item.ok, item.total);
                                                                                            let item_for_click = item.clone();

                                                                                            view! {
                                                                                                <div
                                                                                                    on:click=move |_| set_selected_env.set(Some(item_for_click.clone()))
                                                                                                    style=format!(
                                                                                                        "background: #fff; border-radius: 10px; padding: 10px; box-shadow: 0 1px 2px rgba(0,0,0,0.05); border-top: 3px solid {}; cursor: pointer; min-height: 205px; display: flex; flex-direction: column;",
                                                                                                        if is_healthy { "#10b981" } else { "#ef4444" }
                                                                                                    )
                                                                                                >
                                                                                                    <div style="color: #94a3b8; font-size: 0.54rem; font-weight: 800; text-transform: uppercase; margin-bottom: 2px;">
                                                                                                        {item.customer.clone()}
                                                                                                    </div>

                                                                                                    <div style="color: #1e293b; font-size: 0.90rem; font-weight: 900; margin-bottom: 7px; line-height: 1.05;">
                                                                                                        {item.env_name.clone()}
                                                                                                    </div>

                                                                                                    <div style="display: grid; gap: 2px; margin-bottom: 7px; font-size: 0.68rem; color: #475569;">
                                                                                                        <div>{format!("T: {}", item.total)}</div>
                                                                                                        <div>{format!("OK: {}", item.ok)}</div>
                                                                                                        <div>{format!("Err: {}", item.err)}</div>
                                                                                                    </div>

                                                                                                    <div style="margin-top: auto; display: flex; justify-content: space-between; align-items: center; border-top: 1px solid #f1f5f9; padding-top: 7px;">
                                                                                                        <div>
                                                                                                            <div style=format!(
                                                                                                                "font-weight: 800; font-size: 0.62rem; color: {};",
                                                                                                                if is_healthy { "#059669" } else { "#dc2626" }
                                                                                                            )>
                                                                                                                {if is_healthy { "HEALTHY" } else { "ERROR" }}
                                                                                                            </div>

                                                                                                            <div style="font-size: 0.60rem; color: #64748b;">
                                                                                                                "Click for JSON"
                                                                                                            </div>
                                                                                                        </div>

                                                                                                        <div style=format!(
                                                                                                            "font-size: 1rem; font-weight: 900; color: {};",
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

                                                                        <div style="margin-top: auto; padding-top: 12px;">
                                                                            <div style="background: #f8fafc; border-radius: 10px; padding: 10px; border: 1px solid #e2e8f0;">
                                                                                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 7px;">
                                                                                    <span style="font-size: 0.70rem; font-weight: 800; color: #334155;">
                                                                                        {format!("Group Errors / {} envs", env_count)}
                                                                                    </span>

                                                                                    <span style=format!(
                                                                                        "font-size: 0.76rem; font-weight: 900; color: {};",
                                                                                        if customer_healthy { "#10b981" } else { "#ef4444" }
                                                                                    )>
                                                                                        {format!("{:.1}%", customer_pct)}
                                                                                    </span>
                                                                                </div>

                                                                                <div style="background: #e2e8f0; height: 7px; border-radius: 999px; overflow: hidden;">
                                                                                    <div style=format!(
                                                                                        "height: 100%; width: {:.2}%; background: {}; transition: width 0.4s;",
                                                                                        customer_pct,
                                                                                        if customer_healthy { "#10b981" } else { "#ef4444" }
                                                                                    )></div>
                                                                                </div>

                                                                                <div style="display: flex; justify-content: space-between; margin-top: 7px; font-size: 0.66rem; color: #64748b;">
                                                                                    <span>{format!("OK {}", group.ok)}</span>
                                                                                    <span>{format!("ERR {}", group.err)}</span>
                                                                                    <span>{format!("TOTAL {}", group.total)}</span>
                                                                                </div>
                                                                            </div>
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
                                })
                            }}
                        </Transition>
                    </Show>
                </div>
            </div>
        </>
    }
}

fn main() {
    mount_to_body(|| view! { <App /> })
}