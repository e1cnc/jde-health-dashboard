use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    #[serde(default)] pub customer_name: Option<String>,
    #[serde(default)] pub host_name: Option<String>,
    #[serde(default, alias = "instance_name")] pub group: Option<String>,
    #[serde(default)] pub status: Option<String>,
    #[serde(default, alias = "timestamp")] pub last_sync: Option<String>,
}

#[component]
fn App() -> impl IntoView {
    let (search_query, set_search_query) = create_signal(String::new());
    let (refresh_count, set_refresh_count) = create_signal(0);
    
    let health_data = create_resource(
        move || refresh_count.get(), 
        |_| async move { fetch_health_data().await }
    );

    set_interval(
        move || { set_refresh_count.update(|n| *n += 1); },
        Duration::from_millis(300_000),
    );

    view! {
        <div style="padding: 20px; font-family: sans-serif; background-color: #f0f4f8; min-height: 100vh;">
            <div style="max-width: 1200px; margin: auto; background: white; padding: 25px; border-radius: 12px; box-shadow: 0 4px 15px rgba(0,0,0,0.1);">
                <div style="display: flex; justify-content: space-between; align-items: center; border-bottom: 2px solid #004488; padding-bottom: 10px; margin-bottom: 20px;">
                    <h2 style="color: #004488; margin: 0;">"JDE Global Health Monitor"</h2>
                    <div style="font-size: 0.85em; color: #777;">"Sync Count: " {move || refresh_count.get()}</div>
                </div>

                <input 
                    type="text" 
                    placeholder="Search by Customer or Instance..." 
                    style="width: 100%; padding: 12px; margin-bottom: 20px; border: 1px solid #ccd; border-radius: 8px;"
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
                        <Transition fallback=move || view! { <tr><td colspan="5" style="text-align:center; padding:20px;">"Updating..."</td></tr> }>
                            {move || health_data.get().map(|data| match data {
                                Ok(instances) => {
                                    instances.into_iter()
                                        .filter(|inst| {
                                            let q = search_query.get().to_lowercase();
                                            let customer = inst.customer_name.as_deref().unwrap_or("").to_lowercase();
                                            let group = inst.group.as_deref().unwrap_or("").to_lowercase();
                                            customer.contains(&q) || group.contains(&q)
                                        })
                                        .map(|inst| {
                                            let status_str = inst.status.as_deref().unwrap_or("UNKNOWN");
                                            let is_ok = status_str == "RUNNING" || status_str == "Passed";
                                            let (bg, fg) = if is_ok { ("#e6fffa", "#234e52") } else { ("#fff5f5", "#742a2a") };
                                            view! {
                                                <tr style="border-bottom: 1px solid #edf2f7;">
                                                    <td style="padding: 12px;"><b>{inst.customer_name.unwrap_or_default()}</b></td>
                                                    <td style="padding: 12px;">{inst.host_name.unwrap_or_default()}</td>
                                                    <td style="padding: 12px;">{inst.group.unwrap_or_default()}</td>
                                                    <td style="padding: 12px;">
                                                        <span style=format!("padding: 4px 10px; border-radius: 20px; font-size: 0.85em; font-weight: bold; background: {}; color: {}; border: 1px solid {};", bg, fg, fg)>
                                                            {status_str}
                                                        </span>
                                                    </td>
                                                    <td style="padding: 12px; font-size: 0.8em; color: #4a5568;">{inst.last_sync.unwrap_or_default()}</td>
                                                </tr>
                                            }
                                        }).collect_view()
                                },
                                Err(e) => view! { <tr><td colspan="5" style="color: red; padding: 20px;">{format!("Data Error: {}", e)}</td></tr> }.into_view()
                            })}
                        </Transition>
                    </tbody>
                </table>
            </div>
        </div>
    }
}

async fn fetch_health_data() -> Result<Vec<HealthInstance>, String> {
    let cache_buster = js_sys::Math::random();
    // Use the explicit path to your JSON file hosted on GitHub Pages
    let url = format!("https://e1cnc.github.io/jde-health-dashboard/dashboard_data.json?v={}", cache_buster); 

    let resp = Request::get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.ok() { return Err(format!("HTTP Status: {}", resp.status())); }
    
    resp.json::<Vec<HealthInstance>>().await.map_err(|e| format!("JSON Error: {}", e))
}

fn main() { mount_to_body(|| view! { <App /> }) }