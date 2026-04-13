use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::BTreeMap;
use gloo_timers::callback::Interval;
use web_sys::console;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    #[serde(rename = "customer_name")]
    pub customer_name: Option<String>,
    #[serde(rename = "group_name")] 
    pub server_group: Option<String>,
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

#[component]
fn App() -> impl IntoView {
    let (filter, set_filter) = create_signal(String::new());
    let (refresh_count, set_refresh_count) = create_signal(0);
    let (selected_instance, set_selected_instance) = create_signal(None::<String>);
    
    let health_data = create_resource(move || refresh_count.get(), |_| async move {
        let mut all = Vec::new();
        let targets = vec![("LSJJNEWTR", "dv"), ("LSJJNEWTR", "py")];
        // Ensure this PAR URL is correct for your OCI Ashburn region
        let par_base = "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/";

        for (cust, group) in targets {
            let filename = format!("{}_{}_latest.json", cust, group);
            let url = format!("{}{}", par_base, filename);
            
            match Request::get(&url).send().await {
                Ok(resp) => {
                    if resp.status() == 200 {
                        if let Ok(mut data) = resp.json::<Vec<HealthInstance>>().await {
                            all.append(&mut data);
                        }
                    }
                },
                Err(e) => console::log_1(&format!("Fetch error: {:?}", e).into()),
            }
        }
        all
    });

    core::mem::forget(Interval::new(60_000, move || set_refresh_count.update(|n| *n += 1)));

    view! {
        <div style="padding: 30px; background: #f1f5f9; min-height: 100vh; font-family: sans-serif;">
            
            <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 20px; margin-bottom: 30px;">
                {move || {
                    let data = health_data.get().unwrap_or_default();
                    // Fix: Check for both RUNNING status or Passed healthStatus
                    let running = data.iter().filter(|i| 
                        i.instance_status.as_deref() == Some("RUNNING") || 
                        i.health_status.as_deref() == Some("Passed")
                    ).count();
                    let stopped = data.iter().filter(|i| i.instance_status.as_deref() == Some("STOPPED")).count();
                    
                    view! {
                        <StatusCard title="HEALTHY" count=running color="#22c55e" icon="✔" />
                        <StatusCard title="CRITICAL" count=stopped color="#ef4444" icon="🪲" />
                    }
                }}
            </div>

            <input type="text" 
                placeholder="Filter by environment or instance..." 
                on:input=move |ev| set_filter.set(event_target_value(&ev))
                style="width: 100%; padding: 15px; border-radius: 12px; border: 1px solid #cbd5e1; margin-bottom: 25px;" />

            <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(450px, 1fr)); gap: 25px;">
                <Transition fallback=|| view! { <p>"Syncing with OCI..."</p> }>
                    {move || {
                        let f = filter.get().to_lowercase();
                        let mut groups: BTreeMap<String, Vec<HealthInstance>> = BTreeMap::new();
                        let current_data = health_data.get().unwrap_or_default();

                        for i in current_data {
                            let cust = i.customer_name.clone().unwrap_or_default();
                            let env = i.server_group.clone().unwrap_or_default().to_uppercase();
                            let name = i.instance_name.clone().unwrap_or_default().to_lowercase();
                            if cust.to_lowercase().contains(&f) || env.to_lowercase().contains(&f) || name.contains(&f) {
                                let key = format!("{} | {}", cust, env);
                                groups.entry(key).or_default().push(i);
                            }
                        }

                        groups.into_iter().map(|(title, instances)| {
                            let is_critical = instances.iter().any(|i| i.instance_status.as_deref() == Some("STOPPED"));
                            
                            view! {
                                <div style="background: white; border-radius: 15px; border: 1px solid #e2e8f0; overflow: hidden; box-shadow: 0 4px 6px rgba(0,0,0,0.05);">
                                    <div style=format!(
                                        "background: {}; color: white; padding: 15px 20px; font-weight: bold; display: flex; justify-content: space-between;", 
                                        if is_critical { "#ef4444" } else { "#1e293b" }
                                    )>
                                        <span>{title}</span>
                                        <span style="font-size: 0.8em;">{if is_critical { "CRITICAL" } else { "HEALTHY" }}</span>
                                    </div>
                                    <div style="padding: 10px;">
                                        {instances.into_iter().map(|inst| {
                                            let name = inst.instance_name.clone().unwrap_or_default();
                                            let name_for_click = name.clone();
                                            let name_for_view = name.clone();
                                            
                                            // Determine display status and color
                                            let status = inst.instance_status.clone().unwrap_or_else(|| inst.health_status.clone().unwrap_or_default());
                                            let is_unhealthy = status == "STOPPED" || status == "Failed";
                                            
                                            // Adapted logic from your Axum script: Flatten checks into a string
                                            let mut details_str = inst.message.clone().unwrap_or_default();
                                            if let Some(checks_val) = &inst.checks {
                                                if let Some(checks_arr) = checks_val.as_array() {
                                                    for c in checks_arr {
                                                        let cn = c.get("HealthCheckName").and_then(|v| v.as_str()).unwrap_or("");
                                                        let cr = c.get("Result").and_then(|v| v.as_str()).unwrap_or("");
                                                        details_str.push_str(&format!("{}: {}; ", cn, cr));
                                                    }
                                                }
                                            }

                                            view! {
                                                <div 
                                                    on:click=move |_| {
                                                        if selected_instance.get() == Some(name_for_click.clone()) {
                                                            set_selected_instance.set(None);
                                                        } else {
                                                            set_selected_instance.set(Some(name_for_click.clone()));
                                                        }
                                                    }
                                                    style="cursor: pointer; padding: 12px; border-bottom: 1px solid #f1f5f9;"
                                                >
                                                    <div style="display: flex; justify-content: space-between; align-items: center;">
                                                        <div style="font-weight: 600;">{name}</div>
                                                        <div style=format!(
                                                            "padding: 4px 10px; border-radius: 20px; color: white; font-size: 0.7em; font-weight: bold; background: {};", 
                                                            if is_unhealthy { "#ef4444" } else { "#22c55e" }
                                                        )>
                                                            {status}
                                                        </div>
                                                    </div>
                                                    
                                                    {move || (selected_instance.get() == Some(name_for_view.clone())).then(|| {
                                                        let display_text = details_str.clone();
                                                        view! {
                                                            <div style="margin-top: 10px; padding: 10px; background: #fff7ed; border-left: 4px solid #f97316; font-size: 0.8em; color: #7c2d12;">
                                                                {if display_text.is_empty() { 
                                                                    "No detailed check information available.".to_string() 
                                                                } else { 
                                                                    display_text 
                                                                }}
                                                            </div>
                                                        }
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

fn main() {
    mount_to_body(|| view! { <App /> })
}