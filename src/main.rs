use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::http::Request;

#[derive(Deserialize, Debug, Clone)]
pub struct OCIObject { 
    pub name: String 
}

#[derive(Deserialize, Debug, Clone)]
pub struct OCIListResponse { 
    pub objects: Vec<OCIObject> 
}

const LIST_API_URL: &str = "https://objectstorage.us-ashburn-1.oraclecloud.com/p/2iZ2CfFNkV8LVuzg3LHTaqjseLntrFEtA991Jg9gUUDQjqjP6sSQUqyItWJh15ya/n/id7bn4roxxyb/b/JDE_Monitoring_Data/o/";

async fn debug_fetch_file_names() -> Result<Vec<String>, String> {
    // We add ?format=json to ensure OCI doesn't send XML
    let url = format!("{}?format=json", LIST_API_URL);
    
    let resp = Request::get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Network failure: {}", e))?;

    if !resp.ok() {
        return Err(format!("OCI Server Error: Status {}", resp.status()));
    }

    // Parse the response into our OCIListResponse struct
    let list_data: OCIListResponse = resp.json().await
        .map_err(|e| format!("JSON Struct Mismatch: {}. Check if the API returns 'objects' key.", e))?;

    // Return all names found in the bucket
    Ok(list_data.objects.into_iter().map(|o| o.name).collect())
}

#[component]
fn App() -> impl IntoView {
    let file_resource = create_resource(|| (), |_| async move { debug_fetch_file_names().await });

    view! {
        <div style="padding: 40px; font-family: monospace; background: #1e1e1e; color: #d4d4d4; min-height: 100vh;">
            <h2 style="color: #4ec9b0;">"OCI BUCKET FILE DISCOVERY"</h2>
            <hr style="border: 0; border-top: 1px solid #333; margin-bottom: 20px;"/>
            
            <Transition fallback=|| view! { <p>"Fetching list from Ashburn..."</p> }>
                {move || file_resource.get().map(|res| match res {
                    Err(e) => view! { 
                        <div style="color: #f44747; background: #451a1a; padding: 15px; border-radius: 5px;">
                            <strong>"ERROR: "</strong> {e}
                        </div> 
                    }.into_view(),
                    Ok(names) => {
                        if names.is_empty() {
                            view! { <p style="color: #ce9178;">"Connection successful, but bucket appears to be empty."</p> }.into_view()
                        } else {
                            view! {
                                <div>
                                    <p style="color: #b5cea8;">{format!("Found {} total objects:", names.len())}</p>
                                    <ul style="list-style: none; padding: 0;">
                                        {names.into_iter().map(|name| {
                                            let is_match = name.to_lowercase().ends_with("_latest.json");
                                            view! { 
                                                <li style=format!("padding: 8px; border-bottom: 1px solid #333; color: {};", if is_match { "#9cdcfe" } else { "#6a9955" })>
                                                    {if is_match { "✅ " } else { "❌ " }} {name}
                                                </li> 
                                            }
                                        }).collect_view()}
                                    </ul>
                                </div>
                            }.into_view()
                        }
                    }
                })}
            </Transition>
        </div>
    }
}

fn main() {
    mount_to_body(|| view! { <App /> })
}