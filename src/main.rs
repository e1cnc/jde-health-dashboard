use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::time::Duration;
use chrono::Utc; // Added missing semicolon

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

    // 2. Resource that fetches data triggered by refresh_count
    let (refresh_count, set_refresh_count) = create_signal(0);
    let health_data = create_resource(refresh_count, |_| async move { fetch_health_data().await });

    // 3. Auto-refresh logic (5 minutes)
    set_interval(
        move || {
            set_refresh_count.update(|n| *n += 1);
        },
        Duration::from_millis(300_000),
    );

    view! {
        <div style="padding: 30px; font-family: sans-serif; background-color: #f4f7f9; min-height: 100vh;">
            <div style="max-width: 1200px; margin: auto; background: white; padding: 25px; border-radius: 12px; box-shadow: 0 4px 20px rgba(0,0,0,0.1);">
                
                // Header Area
                <div style="display: flex; justify-content: space-between; align-items: center; border-bottom: 2px solid #004488; padding-bottom: 15px; margin-bottom: 20px;">
                    <h2 style="color: #004488; margin: 0;">"JDE Global Health Monitor"</h2>
                    <div style="font-size: 0.9em; color: #666;">
                        "Updates: " {move || refresh_count.get()} " | Refresh: 5m"
                    </div>
                </div>

                // Filter / Search Bar
                <div style="margin-bottom: 20px;">
                    <input 
                        type="text" 
                        placeholder="Search Customer or Instance..." 
                        style="width: 100%; padding: 12px; border: 1px solid #ccc; border-radius: 8px;"
                        on:input=move |ev| set_search_query.set(event_target_value(&ev))
                        prop:value=search_query
                    />
                </div>

                // Table
                <div style="overflow-x: auto;">
                    <table style="width: 100%; border-collapse: collapse;">
                        <thead>
                            <tr style="background-color: #004488; color: white; text-align: left;">
                                <th style="padding: 12px;">"Customer"</th>
                                <th style="padding: 12px;">"Host"</th>
                                <th style="padding: 12px;">"Instance"</th>
                                <th style="padding: 12px;">"Status"</th>
                                <th style="padding: 12px;">"Last Sync"</th>
                            </tr>
                        </thead>
                        <tbody>
                            <Transition fallback=move || view! { <tr><td colspan="5">"Loading..."</td></tr> }>
                                {move || health_data.get().map(|data| match data {
                                    Ok(instances) => {
                                        let filtered: Vec<_> = instances.into_iter()
                                            .filter(|inst| {
                                                let q = search_query.get().to_lowercase();
                                                inst.customer_name.to_lowercase().contains(&q) || 
                                                inst.group.to_lowercase().contains(&q)
                                            })
                                            .collect();

                                        filtered.into_iter().map(|inst| {
                                            let is_running = inst.status == "RUNNING";
                                            let (bg, txt) = if is_running { ("#d4edda", "#155724") } else { ("#f8d7da", "#721c24") };
                                            
                                            view! {
                                                <tr style="border-bottom: 1px solid #eee;">
                                                    <td style="padding: 12px;"><b>{inst.customer_name}</b></td>
                                                    <td style="padding: 12px;">{inst.host_name}</td>
                                                    <td style="padding: 12px;">{inst.group}</td>
                                                    <td style="padding: 12px;">
                                                        <span style=format!("padding: 4px 12px; border-radius: 15px; background: {}; color: {}; font-weight: bold;", bg, txt)>
                                                            {inst.status}
                                                        </span>
                                                    </td>
                                                    <td style="padding: 12px; font-size: 0.85em;">{inst.last_sync}</td>
                                                </tr>
                                            }
                                        }).collect_view()
                                    },
                                    Err(e) => view! { <tr><td colspan="5" style="color: red;">{format!("Error: {}", e)}</td></tr> }.into_view()
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
    let t = Utc::now().timestamp();
    // Point this to your actual JSON location on GitHub
    let url = format!("./docs/dashboard_data.json?t={}", t); 

    let resp = Request::get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.ok() { return Err(format!("HTTP {}", resp.status())); }
    resp.json::<Vec<HealthInstance>>().await.map_err(|e| e.to_string())
}

fn main() { mount_to_body(|| view! { <App /> }) }