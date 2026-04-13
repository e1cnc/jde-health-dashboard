use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::time::Duration;
use chrono::Utc;

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
    let (search_query, set_search_query) = create_signal(String::new());
    let (refresh_count, set_refresh_count) = create_signal(0);
    
    // Resource updates whenever refresh_count changes
    let health_data = create_resource(refresh_count, |_| async move { fetch_health_data().await });

    // 5-minute auto-refresh
    set_interval(
        move || {
            set_refresh_count.update(|n| *n += 1);
        },
        Duration::from_millis(300_000),
    );

    view! {
        <div style="padding: 20px; font-family: sans-serif; background-color: #f0f2f5; min-height: 100vh;">
            <div style="max-width: 1200px; margin: auto; background: white; padding: 20px; border-radius: 10px; box-shadow: 0 2px 10px rgba(0,0,0,0.1);">
                
                <div style="display: flex; justify-content: space-between; align-items: center; border-bottom: 2px solid #004488; padding-bottom: 10px; margin-bottom: 20px;">
                    <h2 style="color: #004488; margin: 0;">"JDE Global Health Monitor"</h2>
                    <div style="font-size: 0.8em; color: #666;">
                        "Updates: " {move || refresh_count.get()}
                    </div>
                </div>

                <input 
                    type="text" 
                    placeholder="Search Customer or Instance..." 
                    style="width: 100%; padding: 10px; margin-bottom: 20px; border: 1px solid #ddd; border-radius: 5px;"
                    on:input=move |ev| set_search_query.set(event_target_value(&ev))
                    prop:value=search_query
                />

                <table style="width: 100%; border-collapse: collapse;">
                    <thead>
                        <tr style="background-color: #004488; color: white; text-align: left;">
                            <th style="padding: 12px;">"Customer"</th>
                            <th style="padding: 12px;">"Host"</th>
                            <th style="padding: 12px;">"Instance"</th>
                            <th style="padding: 12px;">"Status"</th>
                            <th style="padding: 12px;">"Last Update"</th>
                        </tr>
                    </thead>
                    <tbody>
                        <Transition fallback=move || view! { <tr><td colspan="5">"Syncing..."</td></tr> }>
                            {move || health_data.get().map(|data| match data {
                                Ok(instances) => {
                                    instances.into_iter()
                                        .filter(|inst| {
                                            let q = search_query.get().to_lowercase();
                                            inst.customer_name.to_lowercase().contains(&q) || 
                                            inst.group.to_lowercase().contains(&q)
                                        })
                                        .map(|inst| {
                                            let is_running = inst.status == "RUNNING";
                                            let (bg, fg) = if is_running { ("#d4edda", "#155724") } else { ("#f8d7da", "#721c24") };
                                            view! {
                                                <tr style="border-bottom: 1px solid #eee;">
                                                    <td style="padding: 12px;"><b>{inst.customer_name}</b></td>
                                                    <td style="padding: 12px;">{inst.host_name}</td>
                                                    <td style="padding: 12px;">{inst.group}</td>
                                                    <td style="padding: 12px;">
                                                        <span style=format!("padding: 4px 10px; border-radius: 10px; background: {}; color: {}; font-weight: bold;", bg, fg)>
                                                            {inst.status}
                                                        </span>
                                                    </td>
                                                    <td style="padding: 12px; font-size: 0.8em; color: #666;">{inst.last_sync}</td>
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
    }
}

async fn fetch_health_data() -> Result<Vec<HealthInstance>, String> {
    let t = Utc::now().timestamp();
    // Using relative path for GitHub Pages compatibility
    let url = format!("./docs/dashboard_data.json?t={}", t); 

    let resp = Request::get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.ok() { return Err(format!("HTTP Error {}", resp.status())); }
    resp.json::<Vec<HealthInstance>>().await.map_err(|e| e.to_string())
}

fn main() {
    mount_to_body(|| view! { <App /> })
}