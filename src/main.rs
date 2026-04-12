use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    #[serde(default)]
    pub customer_name: String,
    #[serde(default)]
    pub host_name: String,
    #[serde(default)]
    pub instance_name: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub last_sync: String,
    #[serde(default)]
    pub details: String,
}

#[component]
fn App() -> impl IntoView {
    let health_data = create_resource(|| (), |_| async move { fetch_health_data().await });

    view! {
        <div class="container" style="padding: 20px; font-family: sans-serif; max-width: 1200px; margin: auto;">
            <h2 style="color: #004488; border-bottom: 2px solid #004488; padding-bottom: 10px;">
                "JDE Global Health Check Dashboard"
            </h2>
            <table style="width: 100%; border-collapse: collapse; margin-top: 20px; box-shadow: 0 2px 8px rgba(0,0,0,0.1);">
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
                    <Transition fallback=move || view! { <tr><td colspan="5" style="padding: 30px; text-align: center;">"Loading health data..."</td></tr> }>
                        {move || {
                            health_data.get().map(|data| {
                                match data {
                                    Ok(instances) => {
                                        instances.into_iter().map(|inst| {
                                            let (bg, txt) = if inst.status == "RUNNING" { ("#d4edda", "#155724") } else { ("#f8d7da", "#721c24") };
                                            view! {
                                                <tr style="border-bottom: 1px solid #eee;">
                                                    <td style="padding: 12px; border: 1px solid #ddd;">{inst.customer_name}</td>
                                                    <td style="padding: 12px; border: 1px solid #ddd;">{inst.host_name}</td>
                                                    <td style="padding: 12px; border: 1px solid #ddd;">{inst.instance_name}</td>
                                                    <td style="padding: 12px; border: 1px solid #ddd; background-color: {bg}; color: {txt}; font-weight: bold; text-align: center;">
                                                        {inst.status}
                                                    </td>
                                                    <td style="padding: 12px; border: 1px solid #ddd; font-size: 0.85em; color: #555;">{inst.last_sync}</td>
                                                </tr>
                                            }
                                        }).collect_view()
                                    },
                                    Err(e) => view! { <tr><td colspan="5" style="color: red; padding: 20px; text-align: center;">{format!("Error: {}", e)}</td></tr> }.into_view()
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
    let url = "./dashboard_data.json";
    let resp = Request::get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.ok() { return Err(format!("HTTP {}", resp.status())); }
    resp.json::<Vec<HealthInstance>>().await.map_err(|e| format!("JSON Parse Error: {}", e))
}

fn main() { mount_to_body(|| view! { <App /> }) }