use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HealthInstance {
    // Fields injected by the GitHub Action from the filename
    #[serde(default)]
    pub customer_name: String,

    // Maps "server_alias" from your OCI JSON to the Host column
    #[serde(default, alias = "server_alias")]
    pub host_name: String,

    // Maps "instance_name" or "group_name" to the Group column
    #[serde(default, alias = "instance_name", alias = "group_name")]
    pub instance_name: String,

    #[serde(default)]
    pub status: String,

    // Maps "timestamp" from your OCI JSON to the Last Sync column
    #[serde(default, alias = "timestamp")]
    pub last_sync: String,
}

#[component]
fn App() -> impl IntoView {
    let health_data = create_resource(|| (), |_| async move { fetch_health_data().await });

    view! {
        <div class="container" style="padding: 20px; font-family: sans-serif; max-width: 1200px; margin: auto;">
            <h2 style="color: #004488; border-bottom: 2px solid #004488; padding-bottom: 10px; font-weight: bold;">
                "JDE Global Health Check Dashboard"
            </h2>
            <table style="width: 100%; border-collapse: collapse; margin-top: 20px; box-shadow: 0 4px 12px rgba(0,0,0,0.1); border-radius: 8px; overflow: hidden;">
                <thead>
                    <tr style="background-color: #004488; color: white; text-align: left;">
                        <th style="padding: 15px; border: 1px solid #eee;">"Customer"</th>
                        <th style="padding: 15px; border: 1px solid #eee;">"Host"</th>
                        <th style="padding: 15px; border: 1px solid #eee;">"Group"</th>
                        <th style="padding: 15px; border: 1px solid #eee;">"Status"</th>
                        <th style="padding: 15px; border: 1px solid #eee;">"Last Sync"</th>
                    </tr>
                </thead>
                <tbody>
                    <Transition fallback=move || view! { <tr><td colspan="5" style="padding: 40px; text-align: center; color: #666;">"Fetching live health data..."</td></tr> }>
                        {move || {
                            health_data.get().map(|data| {
                                match data {
                                    Ok(instances) => {
                                        instances.into_iter().map(|inst| {
                                            let (bg, txt) = if inst.status == "RUNNING" { ("#d4edda", "#155724") } else { ("#f8d7da", "#721c24") };
                                            view! {
                                                <tr style="border-bottom: 1px solid #eee;">
                                                    <td style="padding: 12px; border: 1px solid #eee; font-weight: 500;">{inst.customer_name}</td>
                                                    <td style="padding: 12px; border: 1px solid #eee;">{inst.host_name}</td>
                                                    <td style="padding: 12px; border: 1px solid #eee;">{inst.instance_name}</td>
                                                    <td style="padding: 12px; border: 1px solid #eee; text-align: center;">
                                                        <span style="padding: 6px 12px; border-radius: 20px; background-color: {bg}; color: {txt}; font-size: 0.9em; font-weight: bold;">
                                                            {inst.status}
                                                        </span>
                                                    </td>
                                                    <td style="padding: 12px; border: 1px solid #eee; font-size: 0.85em; color: #777;">{inst.last_sync}</td>
                                                </tr>
                                            }
                                        }).collect_view()
                                    },
                                    Err(e) => view! { <tr><td colspan="5" style="color: #c00; padding: 20px; text-align: center;">{format!("Error loading data: {}", e)}</td></tr> }.into_view()
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
    // Relative path ensures it works in the GitHub Pages subfolder
    let url = "./dashboard_data.json";
    let resp = Request::get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.ok() { return Err(format!("HTTP Error: {}", resp.status())); }
    resp.json::<Vec<HealthInstance>>().await.map_err(|e| format!("Parsing failed: {}", e))
}

fn main() { mount_to_body(|| view! { <App /> }) }