use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    pub customer_name: String,
    pub host_name: String,
    pub instance_name: String,
    pub status: String,
    pub last_sync: String,
}

#[component]
fn App() -> impl IntoView {
    // Resource to fetch data asynchronously
    let health_data = create_resource(|| (), |_| async move { fetch_health_data().await });

    view! {
        <div class="container" style="padding: 20px; font-family: sans-serif;">
            <h2 style="color: #004488; border-bottom: 2px solid #004488;">"JDE Global Health Dashboard"</h2>
            <table style="width: 100%; border-collapse: collapse; margin-top: 20px;">
                <thead>
                    <tr style="background-color: #004488; color: white; text-align: left;">
                        <th style="padding: 12px; border: 1px solid #ddd;">"Customer"</th>
                        <th style="padding: 12px; border: 1px solid #ddd;">"Host"</th>
                        <th style="padding: 12px; border: 1px solid #ddd;">"Group"</th>
                        <th style="padding: 12px; border: 1px solid #ddd;">"Status"</th>
                        <th style="padding: 12px; border: 1px solid #ddd;">"Last Sync"</th>
                    </tr>
                </thead>
                <tbody>
                    <Transition fallback=move || view! { <tr><td colspan="5">"Loading health data..."</td></tr> }>
                        {move || {
                            health_data.get().map(|data| {
                                match data {
                                    Ok(instances) => {
                                        instances.into_iter().map(|inst| {
                                            let status_color = if inst.status == "RUNNING" { "#d4edda" } else { "#f8d7da" };
                                            let text_color = if inst.status == "RUNNING" { "#155724" } else { "#721c24" };
                                            
                                            view! {
                                                <tr>
                                                    <td style="padding: 10px; border: 1px solid #ddd;">{inst.customer_name}</td>
                                                    <td style="padding: 10px; border: 1px solid #ddd;">{inst.host_name}</td>
                                                    <td style="padding: 10px; border: 1px solid #ddd;">{inst.instance_name}</td>
                                                    <td style="padding: 10px; border: 1px solid #ddd; background-color: {status_color}; color: {text_color}; font-weight: bold;">
                                                        {inst.status}
                                                    </td>
                                                    <td style="padding: 10px; border: 1px solid #ddd; font-size: 0.85em;">{inst.last_sync}</td>
                                                </tr>
                                            }
                                        }).collect_view()
                                    },
                                    Err(e) => view! { <tr><td colspan="5" style="color: red;">{format!("Error loading data: {}", e)}</td></tr> }.into_view()
                                }
                            })
                        }}
                    </Transition>
                </tbody>
            </table>
        </div>
    }
}

async fn fetch_health_data() -> Result<Vec<HealthInstance>, String> {
    // Using "./" ensures it looks in the current subfolder (/docs/)
    let url = "./dashboard_data.json";
    
    let resp = Request::get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.ok() {
        return Err(format!("Request failed: {} {}", resp.status(), resp.status_text()));
    }

    let data: Vec<HealthInstance> = resp.json()
        .await
        .map_err(|e| format!("JSON Parse Error: {}", e))?;
        
    Ok(data)
}

fn main() {
    mount_to_body(|| view! { <App /> })
}