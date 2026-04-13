use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::collections::HashMap;
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
    pub timestamp: Option<String>,
}

#[component]
fn App() -> impl IntoView {
    let (filter, set_filter) = create_signal(String::new());
    let (refresh_count, set_refresh_count) = create_signal(0);
    
    // Resource to fetch data from your OCI bucket
    let health_data = create_resource(move || refresh_count.get(), |_| async move {
        // Add your known "latest" filenames here
        let files = vec!["LSJJNEWTR_DEDE1_latest.json", "LSJJNEWTR_PD_latest.json"];
        let mut all = Vec::new();
        
        for f in files {
            let url = format!("https://ixxxx/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/{}?t={}", f, js_sys::Date::now());
            if let Ok(r) = Request::get(&url).send().await {
                if let Ok(mut d) = r.json::<Vec<HealthInstance>>().await { 
                    all.append(&mut d); 
                }
            }
        }
        all
    });

    // Auto-refresh every 5 minutes
    core::mem::forget(Interval::new(300_000, move || set_refresh_count.update(|n| *n += 1)));

    view! {
        <div style="padding: 30px; background: #f8fafc; min-height: 100vh; font-family: 'Segoe UI', system-ui, sans-serif;">
            
            // --- STATS CARDS ---
            <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 20px; margin-bottom: 30px;">
                {move || {
                    let data = health_data.get().unwrap_or_default();
                    let passed = data.iter().filter(|i| i.health_status.as_deref() == Some("Passed")).count();
                    let failed = data.iter().filter(|i| i.health_status.as_deref() == Some("Failed")).count();
                    let stopped = data.iter().filter(|i| i.instance_status.as_deref() != Some("Running")).count();
                    
                    view! {
                        <StatusCard title="HEALTHY" count=passed color="#22c55e" icon="✔" />
                        <StatusCard title="FAILED" count=failed color="#f59e0b" icon="⚠" />
                        <StatusCard title="STOPPED" count=stopped color="#3b82f6" icon="ℹ" />
                        <StatusCard title="CRITICAL" count=failed color="#ef4444" icon="🪲" />
                    }
                }}
            </div>

            // --- SEARCH BAR ---
            <div style="margin-bottom: 25px;">
                <input type="text" 
                    placeholder="Search Customer, Group, or Instance..." 
                    prop:value=filter
                    on:input=move |ev| set_filter.set(event_target_value(&ev))
                    style="width: 100%; padding: 14px; border-radius: 10px; border: 1px solid #cbd5e1; font-size: 1rem;" />
            </div>

            // --- MAIN GRID ---
            <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(380px, 1fr)); gap: 20px;">
                <Transition fallback=|| view! { <p>"Syncing with OCI Storage..."</p> }>
                    {move || {
                        let f = filter.get().to_lowercase();
                        let mut groups: HashMap<String, Vec<HealthInstance>> = HashMap::new();
                        
                        for i in health_data.get().unwrap_or_default() {
                            let c_name = i.customer_name.clone().unwrap_or_default();
                            let i_name = i.instance_name.clone().unwrap_or_default();
                            
                            if c_name.to_lowercase().contains(&f) || i_name.to_lowercase().contains(&f) {
                                groups.entry(c_name).or_default().push(i);
                            }
                        }

                        groups.into_iter().map(|(cust, instances)| view! {
                            <div style="background: white; border-radius: 12px; border: 1px solid #e2e8f0; box-shadow: 0 4px 6px -1px rgba(0,0,0,0.05); overflow: hidden;">
                                <div style="background: #1e293b; color: white; padding: 15px 20px; font-weight: bold; display: flex; justify-content: space-between;">
                                    <span>{cust}</span>
                                    <span style="font-size: 0.8em; opacity: 0.7;">{instances.len()} " Instances"</span>
                                </div>
                                <div style="padding: 10px;">
                                    {instances.into_iter().map(|inst| {
                                        let is_failed = inst.health_status.as_deref() == Some("Failed");
                                        let is_stopped = inst.instance_status.as_deref() != Some("Running");
                                        
                                        view! {
                                            <div style="display: flex; justify-content: space-between; padding: 12px; border-bottom: 1px solid #f1f5f9; align-items: center;">
                                                <div style="max-width: 70%;">
                                                    <div style="font-weight: 600; color: #334155;">{inst.instance_name}</div>
                                                    <div style="font-size: 0.75em; color: #64748b; font-family: monospace;">{inst.details}</div>
                                                </div>
                                                <div style=format!(
                                                    "color: white; padding: 4px 10px; border-radius: 6px; font-size: 0.7em; font-weight: bold; background: {};", 
                                                    if is_failed { "#ef4444" } else if is_stopped { "#3b82f6" } else { "#22c55e" }
                                                )>
                                                    {if is_failed { "FAILED" } else if is_stopped { "STOPPED" } else { "HEALTHY" }}
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
        <div style=format!("background: {}; color: white; padding: 20px; border-radius: 12px; transition: transform 0.2s;", color)>
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px;">
                <span style="font-weight: 800; font-size: 0.85em; letter-spacing: 0.05em;">{title}</span>
                <span style="font-size: 1.2em;">{icon}</span>
            </div>
            <h1 style="margin: 0; font-size: 2.8em; font-weight: 900;">{count}</h1>
            <div style="margin-top: 5px; font-size: 0.7em; opacity: 0.8;">"Live Status"</div>
        </div>
    }
}

// THE MISSING ENTRY POINT
fn main() {
    mount_to_body(|| view! { <App /> })
}