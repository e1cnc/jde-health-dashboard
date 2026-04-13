use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::BTreeMap;
use gloo_timers::callback::Interval;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    pub customer_name: Option<String>,
    pub group_name: Option<String>,
    #[serde(rename = "instanceName")]
    pub instance_name: Option<String>,
    #[serde(rename = "instanceStatus")]
    pub instance_status: Option<String>,
    #[serde(rename = "healthStatus")]
    pub health_status: Option<String>,
    #[serde(rename = "message")]
    pub message: Option<String>,
    #[serde(rename = "instanceHealthChecks")]
    pub checks: Option<serde_json::Value>,
}

// Helper to extract Meta from Filename: "CUSTOMER_GROUP_latest.json"
fn parse_filename(name: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = name.split('_').collect();
    if parts.len() >= 2 {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else {
        None
    }
}

#[component]
fn App() -> impl IntoView {
    let (filter, set_filter) = create_signal(String::new());
    let (refresh_count, set_refresh_count) = create_signal(0);
    let (selected_instance, set_selected_instance) = create_signal(None::<String>);
    
    let health_data = create_resource(move || refresh_count.get(), |_| async move {
        let mut all_data = Vec::new();
        // Use your Bucket-level PAR URL here
        let bucket_url = "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/";

        // 1. List all objects in the bucket
        if let Ok(resp) = Request::get(bucket_url).send().await {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(objects) = json.get("objects").and_then(|o| o.as_array()) {
                    for obj in objects {
                        if let Some(name) = obj.get("name").and_then(|n| n.as_str()) {
                            // Only process files ending in _latest.json
                            if name.ends_with("_latest.json") {
                                if let Some((cust, grp)) = parse_filename(name) {
                                    let file_url = format!("{}{}", bucket_url, name);
                                    
                                    // 2. Fetch the actual health data for this file
                                    if let Ok(file_resp) = Request::get(&file_url).send().await {
                                        if let Ok(mut instances) = file_resp.json::<Vec<HealthInstance>>().await {
                                            for i in instances.iter_mut() {
                                                i.customer_name = Some(cust.clone());
                                                i.group_name = Some(grp.clone().to_uppercase());
                                            }
                                            all_data.append(&mut instances);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        all_data
    });

    core::mem::forget(Interval::new(60_000, move || set_refresh_count.update(|n| *n += 1)));

    view! {
        <div style="padding: 30px; background: #f1f5f9; min-height: 100vh; font-family: sans-serif;">
            
            // SUMMARY CARDS
            <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 20px; margin-bottom: 30px;">
                {move || {
                    let data = health_data.get().unwrap_or_default();
                    let healthy = data.iter().filter(|i| 
                        i.health_status.as_deref() == Some("Passed") || 
                        i.instance_status.as_deref() == Some("RUNNING")
                    ).count();
                    let critical = data.iter().filter(|i| i.instance_status.as_deref() == Some("STOPPED")).count();
                    view! {
                        <StatusCard title="HEALTHY" count=healthy color="#22c55e" icon="✔" />
                        <StatusCard title="CRITICAL" count=critical color="#ef4444" icon="🪲" />
                    }
                }}
            </div>

            <input type="text" placeholder="Search Customer, Environment, or Instance..." 
                on:input=move |ev| set_filter.set(event_target_value(&ev))
                style="width: 100%; padding: 15px; border-radius: 12px; border: 1px solid #cbd5e1; margin-bottom: 25px;" />

            <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(450px, 1fr)); gap: 25px;">
                <Transition fallback=|| view! { <p>"Discovering instances in OCI Bucket..."</p> }>
                    {move || {
                        let f = filter.get().to_lowercase();
                        let mut groups: BTreeMap<String, Vec<HealthInstance>> = BTreeMap::new();
                        let current_data = health_data.get().unwrap_or_default();

                        for i in current_data {
                            let cust = i.customer_name.clone().unwrap_or_default();
                            let env = i.group_name.clone().unwrap_or_default();
                            if cust.to_lowercase().contains(&f) || env.to_lowercase().contains(&f) {
                                let key = format!("{} | {}", cust, env);
                                groups.entry(key).or_default().push(i);
                            }
                        }

                        groups.into_iter().map(|(title, instances)| {
                            view! {
                                <div style="background: white; border-radius: 15px; border: 1px solid #e2e8f0; overflow: hidden; box-shadow: 0 4px 6px rgba(0,0,0,0.05);">
                                    <div style="background: #1e293b; color: white; padding: 15px 20px; font-weight: bold;">{title}</div>
                                    <div style="padding: 10px;">
                                        {instances.into_iter().map(|inst| {
                                            let name = inst.instance_name.clone().unwrap_or_default();
                                            let name_id = name.clone();
                                            let status = inst.instance_status.clone().unwrap_or_else(|| inst.health_status.clone().unwrap_or_default());
                                            let is_healthy = status == "RUNNING" || status == "Passed";
                                            
                                            // Extract check details
                                            let mut details = inst.message.clone().unwrap_or_default();
                                            if let Some(checks) = inst.checks.as_ref().and_then(|v| v.as_array()) {
                                                for c in checks {
                                                    let cn = c.get("HealthCheckName").and_then(|v| v.as_str()).unwrap_or("");
                                                    let cr = c.get("Result").and_then(|v| v.as_str()).unwrap_or("");
                                                    details.push_str(&format!("{}: {}; ", cn, cr));
                                                }
                                            }

                                            view! {
                                                <div on:click=move |_| set_selected_instance.update(|s| *s = if *s == Some(name_id.clone()) { None } else { Some(name_id.clone()) }) 
                                                     style="cursor: pointer; padding: 12px; border-bottom: 1px solid #f1f5f9;">
                                                    <div style="display: flex; justify-content: space-between; align-items: center;">
                                                        <div style="font-weight: 600;">{name.clone()}</div>
                                                        <div style=format!("color: white; padding: 4px 12px; border-radius: 20px; font-size: 0.7em; font-weight: bold; background: {};", if is_healthy { "#22c55e" } else { "#ef4444" })>
                                                            {status}
                                                        </div>
                                                    </div>
                                                    {move || (selected_instance.get() == Some(name.clone())).then(|| {
                                                        view! { <div style="font-size: 0.75em; color: #475569; margin-top: 8px; line-height: 1.5; padding: 8px; background: #f8fafc;">{details.clone()}</div> }
                                                    })}
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>
                                </div>
                            }
                        }).collect_view()
                    }}
                </Transition>
            </div>
        </div>
    }
}

#[component]
fn StatusCard(title: &'static str, count: usize, color: &'static str, icon: &'static str) -> impl IntoView {
    view! {
        <div style=format!("background: {}; color: white; padding: 25px; border-radius: 15px;", color)>
            <div style="display: flex; justify-content: space-between; font-weight: bold; opacity: 0.8;">
                <span>{title}</span><span>{icon}</span>
            </div>
            <h1 style="margin: 10px 0 0 0; font-size: 3em;">{count}</h1>
        </div>
    }
}

fn main() { mount_to_body(|| view! { <App /> }) }