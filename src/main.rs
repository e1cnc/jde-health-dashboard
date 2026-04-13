use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    #[serde(default)]
    pub customer_name: String,
    #[serde(default)]
    pub host_name: String,
    #[serde(default, alias = "instance_name")]
    pub group: String,
    #[serde(default)]
    pub status: String,
    #[serde(default, alias = "timestamp")]
    pub last_sync: String,
    #[serde(default)]
    pub details: String,
}

#[component]
fn App() -> impl IntoView {
    // 1. State for search/filtering
    let (search_query, set_search_query) = create_signal(String::new());

    // 2. Resource that fetches data
    // We use a manual 'refetch' trigger to handle the 5-minute refresh
    let (refresh_count, set_refresh_count) = create_signal(0);
    let health_data = create_resource(refresh_count, |_| async move { fetch_health_data().await });

    // 3. Auto-refresh logic (5 minutes = 300,000 ms)
    set_interval(
        move || {
            set_refresh_count.update(|n| *n += 1);
        },
        Duration::from_millis(300_000),
    );

    view! {
        <div style="padding: 30px; font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; background-color: #f4f7f9; min-height: 100vh;">
            <div style="max-width: 1200px; margin: auto; background: white; padding: 25px; border-radius: 12px; box-shadow: 0 4px 20px rgba(0,0,0,0.08);">
                
                // Header Area
                <div style="display: flex; justify-content: space-between; align-items: center; border-bottom: 2px solid #004488; padding-bottom: 15px; margin-bottom: 20px;">
                    <h2 style="color: #004488; margin: 0; font-size: 24px;">"JDE Global Health Monitor"</h2>
                    <div style="font-size: 0.9em; color: #666;">
                        "Auto-refreshing every 5 mins | Updates: " {move || refresh_count.get()}
                    </div>
                </div>

                // Filter / Search Bar
                <div style="margin-bottom: 20px;">
                    <input 
                        type="text" 
                        placeholder="Search by Customer or Instance Name..." 
                        style="width: 100%; padding: 12px; border: 1px solid #ccc; border-radius: 8px; font-size: 16px; outline: none;"
                        on:input=move |ev| set_search_query.set(event_target_value(&ev))
                        prop:value=search_query
                    />
                </div>

                // Table
                <div style="overflow-x: auto;">
                    <table style="width: 100%; border-collapse: collapse; text-align: left;">
                        <thead>
                            <tr style="background-color: #004488; color: white;">
                                <th style="padding: 15px; border: 1px solid #eee;">"Customer"</th>
                                <th style="padding: 15px; border: 1px solid #eee;">"Host"</th>
                                <th style="padding: 15px; border: 1px solid #eee;">"Instance"</th>
                                <th style="padding: 15px; border: 1px solid #eee;">"Status"</th>
                                <th style="padding: 15px; border: 1px solid #eee;">"Last Sync"</th>
                            </tr>
                        </thead>
                        <tbody>
                            <Transition fallback=move || view! { <tr><td colspan="5" style="text-align: center; padding: 50px;">"Loading health data..."</td></tr> }>
                                {move || health_data.get().map(|data| match data {
                                    Ok(instances) => {
                                        let filtered: Vec<_> = instances.into_iter()
                                            .filter(|inst| {
                                                let search = search_query.get().to_lowercase();
                                                inst.customer_name.to_lowercase().contains(&search) || 
                                                inst.group.to_lowercase().contains(&search)
                                            })
                                            .collect();

                                        if filtered.is_empty() {
                                            return view! { <tr><td colspan="5" style="text-align: center; padding: 20px;">"No matching records found."</td></tr> }.into_view();
                                        }

                                        filtered.into_iter().map(|inst| {
                                            // Red/Green Status Logic
                                            let is_running = inst.status == "RUNNING";
                                            let (bg, txt) = if is_running { ("#d4edda", "#155724") } else { ("#f8d7da", "#721c24") };
                                            
                                            view! {
                                                <tr style="border-bottom: 1px solid #eee; hover: background-color: #f9f9f9;">
                                                    <td style="padding: 12px; border: 1px solid #eee; font-weight: bold;">{inst.customer_name}</td>
                                                    <td style="padding: 12px; border: 1px solid #eee;">{inst.host_name}</td>
                                                    <td style="padding: 12px; border: 1px solid #eee;">{inst.group}</td>
                                                    <td style="padding: 12px; border: 1px solid #eee; text-align: center;">
                                                        <span style=format!("padding: 6px 14px; border-radius: 20px; font-weight: bold; background-color: {}; color: {}; border: 1px solid {};", bg, txt, txt)>
                                                            {inst.status}
                                                        </span>
                                                    </td>
                                                    <td style="padding: 12px; border: 1px solid #eee; font-size: 0.85em; color: #555;">{inst.last_sync}</td>
                                                </tr>
                                            }
                                        }).collect_view()
                                    },
                                    Err(e) => view! { <tr><td colspan="5" style="color: red; text-align: center; padding: 20px;">{format!("Error: {}", e)}</td></tr> }.into_view()
                                })}
                            </Transition>
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
    }
}

async fn fetch_health_data() -> Result<Vec<HealthInstance>, String> {
    // Add a cache-buster timestamp to force GitHub to serve the absolute latest JSON
    let timestamp = Utc::now().timestamp();
    let url = format!("./docs/dashboard_data.json?t={}", timestamp); 

    let resp = Request::get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.ok() { return Err(format!("HTTP Error: {}", resp.status())); }
    resp.json::<Vec<HealthInstance>>().await.map_err(|e| format!("Parsing Error: {}", e))
}

use chrono::Utc;
fn main() { mount_to_body(|| view! { <App /> }) }