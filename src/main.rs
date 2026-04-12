use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct HealthRecord {
    customer: String,
    host: String,
    group: String,
    timestamp: String,
    filename: String,
}

#[component]
fn Dashboard() -> impl IntoView {
    // Create a resource that fetches our consolidated JSON
    let health_data = create_resource(|| (), |_| async move {
        let resp = Request::get("./consolidated_health.json")
            .send()
            .await
            .unwrap()
            .json::<Vec<HealthRecord>>()
            .await;
        resp.unwrap_or_default()
    });

    view! {
        <div style="font-family: sans-serif; padding: 20px;">
            <h2 style="color: #004085; border-bottom: 2px solid #004085;">"JDE Global Health (WASM)"</h2>
            <table style="width: 100%; border-collapse: collapse; background: white; box-shadow: 0 2px 5px rgba(0,0,0,0.1);">
                <thead>
                    <tr style="background: #004085; color: white;">
                        <th style="padding: 12px; text-align: left;">"Customer"</th>
                        <th style="padding: 12px; text-align: left;">"Host"</th>
                        <th style="padding: 12px; text-align: left;">"Group"</th>
                        <th style="padding: 12px; text-align: left;">"Last Sync"</th>
                    </tr>
                </thead>
                <tbody>
                    {move || match health_data.get() {
                        None => view! { <tr><td colspan="4">"Loading data..."</td></tr> }.into_view(),
                        Some(records) => {
                            records.into_iter().map(|rec| {
                                view! {
                                    <tr style="border-bottom: 1px solid #eee;">
                                        <td style="padding: 12px;"><b>{rec.customer}</b></td>
                                        <td style="padding: 12px;">{rec.host}</td>
                                        <td style="padding: 12px;">{rec.group}</td>
                                        <td style="padding: 12px; color: #666;">{rec.timestamp}</td>
                                    </tr>
                                }
                            }).collect_view()
                        }
                    }}
                </tbody>
            </table>
        </div>
    }
}

fn main() {
    mount_to_body(|| view! { <Dashboard /> })
}