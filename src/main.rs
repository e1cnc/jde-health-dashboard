use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::BTreeMap;
use gloo_timers::callback::Interval;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    pub customer_name: Option<String>,
    #[serde(alias = "group_name")] 
    pub server_group: Option<String>,
    pub instance_name: Option<String>,
    pub instance_status: Option<String>,
    pub health_status: Option<String>,
    pub details: Option<String>,
}

#[component]
fn App() -> impl IntoView {
    let (filter, set_filter) = create_signal(String::new());
    let (refresh_count, set_refresh_count) = create_signal(0);
    
    let health_data = create_resource(move || refresh_count.get(), |_| async move {
        let mut all = Vec::new();
        // You can add your specific environment-named files here
        let files = vec!["LSJJNEWTR_dv_latest.json", "LSJJNEWTR_py_latest.json"];
        
        let par_prefix = "https://objectstorage.us-ashburn-1.oraclecloud.com/p/YOUR_KEY/n/YOUR_NS/b/JDE_Monitoring_Data/o/";

        for f in files {
            let url = format!("{}{}.json?t={}", par_prefix, f, js_sys::Date::now());
            if let Ok(r) = Request::get(&url).send().await {
                if let Ok(mut d) = r.json::<Vec<HealthInstance>>().await { 
                    all.append(&mut d); 
                }
            }
        }
        all
    });

    core::mem::forget(Interval::new(300_000, move || set_refresh_count.update(|n| *n += 1)));

    view! {
        <div style="padding: 30px; background: #f1f5f9; min-height: 100vh; font-family: sans-serif;">
            
            // --- TOP STATS CARDS ---
            <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 20px; margin-bottom: 30px;">
                {move || {
                    let data = health_data.get().unwrap_or_default();
                    let passed = data.iter().filter(|i| i.health_status.as_deref() == Some("Passed")).count();
                    let failed = data.iter().filter(|i| i.health_status.as_deref() == Some("Failed")).count();
                    
                    view! {
                        <StatusCard title="HEALTHY" count=passed color="#22c55e" icon="✔" />
                        <StatusCard title="CRITICAL" count=failed color="#ef4444" icon="🪲" />
                    }
                }}
            </div>

            // --- SEARCH/FILTER ---
            <input type="text" 
                placeholder="Search by Customer or Environment (dv, py, pd)..." 
                on:input=move |ev| set_filter.set(event_target_value(&ev))
                style="width: 100%; padding: 15px; border-radius: 12px; border: 1px solid #cbd5e1; margin-bottom: 25px; font-size: 1rem;" />

            // --- ENVIRONMENT GROUPS ---
            <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(400px, 1fr)); gap: 20px;">
                <Transition fallback=|| view! { <p>"Syncing Environments..."</p> }>
                    {move || {
                        let f = filter.get().to_lowercase();
                        let mut groups: BTreeMap<String, Vec<HealthInstance>> = BTreeMap::new();
                        
                        for i in health_data.get().unwrap_or_default() {
                            let cust = i.customer_name.clone().unwrap_or_default();
                            let env = i.server_group.clone().unwrap_or_default().to_uppercase();
                            
                            if cust.to_lowercase().contains(&f) || env.to_lowercase().contains(&f) {
                                // Key format "CUSTOMER | ENVIRONMENT" ensures correct sorting
                                let key = format!("{} | {}", cust, env);
                                groups.entry(key).or_default().push(i);
                            }
                        }

                        groups.into_iter().map(|(title, instances)| view! {
                            <div style="background: white; border-radius: 15px; border: 1px solid #e2e8f0; box-shadow: 0 4px 6px rgba(0,0,0,0.05); overflow: hidden;">
                                <div style="background: #1e293b; color: white; padding: 15px 20px; font-weight: bold; letter-spacing: 0.05em;">
                                    {title}
                                </div>
                                <div style="padding: 10px;">
                                    {instances.into_iter().map(|inst| {
                                        let is_failed = inst.health_status.as_deref() == Some("Failed");
                                        view! {
                                            <div style="display: flex; justify-content: space-between; padding: 12px; border-bottom: 1px solid #f1f5f9; align-items: center;">
                                                <div>
                                                    <div style="font-weight: 600; color: #1e293b;">{inst.instance_name}</div>
                                                    <div style="font-size: 0.75em; color: #64748b; font-family: monospace;">{inst.details}</div>
                                                </div>
                                                <div style=format!(
                                                    "padding: 5px 12px; border-radius: 20px; font-size: 0.7em; font-weight: 800; color: white; background: {};", 
                                                    if is_failed { "#ef4444" } else { "#22c55e" }
                                                )>
                                                    {if is_failed { "CRITICAL" } else { "HEALTHY" }}
                                                </div>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            </div>
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
        <div style=format!("background: {}; color: white; padding: 25px; border-radius: 15px; box-shadow: 0 10px 15px -3px rgba(0,0,0,0.1);", color)>
            <div style="display: flex; justify-content: space-between; align-items: center; opacity: 0.8; font-weight: bold;">
                <span>{title}</span><span>{icon}</span>
            </div>
            <h1 style="margin: 10px 0 0 0; font-size: 3em;">{count}</h1>
        </div>
    }
}

fn main() { mount_to_body(|| view! { <App /> }) }