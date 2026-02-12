use anyhow::{anyhow, Result};
use pocketclaw_core::types::{Message, Role};
use reqwest::{Client, Response};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::error;
use yup_oauth2::{ServiceAccountAuthenticator, AccessToken};
use yup_oauth2::authenticator::Authenticator;
use yup_oauth2::hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::connect::HttpConnector;


use crate::session::Session;

#[derive(Clone)]
pub struct SheetsClient {
    auth: Arc<Authenticator<HttpsConnector<HttpConnector>>>,
    client: Client,
    spreadsheet_id: String,
}

#[derive(Deserialize)]
struct SheetValuesResponse {
    values: Option<Vec<Vec<Value>>>,
}

#[derive(Deserialize)]
struct SpreadsheetResponse {
    sheets: Option<Vec<Sheet>>,
}

#[derive(Deserialize)]
struct Sheet {
    properties: Option<SheetProperties>,
}

#[derive(Deserialize)]
struct SheetProperties {
    title: Option<String>,
}

impl SheetsClient {
    pub async fn new(service_account_json: String, spreadsheet_id: String) -> Result<Self> {
        let creds = yup_oauth2::parse_service_account_key(service_account_json)
            .map_err(|e| anyhow!("Failed to parse service account JSON: {}", e))?;

        let auth = ServiceAccountAuthenticator::builder(creds)
            .build()
            .await
            .map_err(|e| anyhow!("Failed to create authenticator: {}", e))?;

        Ok(Self {
            auth: Arc::new(auth),
            client: Client::new(),
            spreadsheet_id,
        })
    }

    async fn get_token(&self) -> Result<String> {
        let token: AccessToken = self.auth.token(&["https://www.googleapis.com/auth/spreadsheets"]).await?;
        Ok(token.token().map(|s| s.to_string()).ok_or(anyhow!("No token string"))?)
    }

    pub async fn load_session(&self, session_key: &str) -> Result<Option<Session>> {
        let valid_sheet_name = sanitize_sheet_name(session_key);
        let token = self.get_token().await?;

        // Check if sheet exists
        let url = format!("https://sheets.googleapis.com/v4/spreadsheets/{}", self.spreadsheet_id);
        let res: Response = self.client.get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;
        
        if !res.status().is_success() {
             return Err(anyhow!("Failed to get spreadsheet: {}", res.status()));
        }
        
        let spreadsheet: SpreadsheetResponse = res.json().await?;
        let sheet_exists = spreadsheet.sheets.unwrap_or_default().iter().any(|s| {
            s.properties.as_ref().map(|p| p.title.as_deref() == Some(&valid_sheet_name)).unwrap_or(false)
        });

        if !sheet_exists {
            return Ok(None);
        }

        // Read values
        let range = format!("{}!A:C", valid_sheet_name);
        let url = format!("https://sheets.googleapis.com/v4/spreadsheets/{}/values/{}", self.spreadsheet_id, range);
        let res: Response = self.client.get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        if !res.status().is_success() {
             return Err(anyhow!("Failed to read values: {}", res.status()));
        }
        
        let data: SheetValuesResponse = res.json().await?;
        let rows = data.values.unwrap_or_default();
        
        let mut history = Vec::new();
        let summary = None;

        for (i, row) in rows.iter().enumerate() {
            if i == 0 { continue; } // Skip header
            
            if row.len() >= 2 {
                let role_str = row[0].as_str().unwrap_or_default();
                let content = row[1].as_str().unwrap_or_default().to_string();
                
                let role = match role_str.to_lowercase().as_str() {
                    "user" => Role::User,
                    "assistant" => Role::Assistant,
                    "system" => Role::System,
                    "tool" => Role::Tool,
                    _ => Role::User,
                };

                let metadata = if row.len() >= 3 {
                    serde_json::from_str(row[2].as_str().unwrap_or("{}")).unwrap_or_default()
                } else {
                    Default::default()
                };

                let mut msg = Message::new(
                    "sheets",
                    session_key,
                    role,
                    &content,
                );
                msg.metadata = metadata;
                history.push(msg);
            }
        }

        Ok(Some(Session { history, summary }))
    }

    pub async fn ensure_sheet_exists(&self, session_key: &str) -> Result<()> {
        let valid_sheet_name = sanitize_sheet_name(session_key);
        let token = self.get_token().await?;

        // Check existence (optimization: cache this? no, reliability first)
        let url = format!("https://sheets.googleapis.com/v4/spreadsheets/{}", self.spreadsheet_id);
        let res: Response = self.client.get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;
        let res = res.error_for_status()?;
            
        let spreadsheet: SpreadsheetResponse = res.json().await?;
        let sheet_exists = spreadsheet.sheets.unwrap_or_default().iter().any(|s| {
            s.properties.as_ref().map(|p| p.title.as_deref() == Some(&valid_sheet_name)).unwrap_or(false)
        });

        if !sheet_exists {
            // Create sheet
            let body = json!({
                "requests": [{
                    "addSheet": {
                        "properties": {
                            "title": valid_sheet_name
                        }
                    }
                }]
            });
            
            let url = format!("https://sheets.googleapis.com/v4/spreadsheets/{}:batchUpdate", self.spreadsheet_id);
            let res: Response = self.client.post(&url)
                .header("Authorization", format!("Bearer {}", token))
                .json(&body)
                .send()
                .await?;
            res.error_for_status()?;
                
            // Add Header
            let body = json!({
                "values": [["ROLE", "CONTENT", "METADATA"]]
            });
            let range = format!("{}!A1:C1", valid_sheet_name);
            let url = format!("https://sheets.googleapis.com/v4/spreadsheets/{}/values/{}:append?valueInputOption=RAW", self.spreadsheet_id, range);
            
            let res: Response = self.client.post(&url)
                .header("Authorization", format!("Bearer {}", token))
                .json(&body)
                .send()
                .await?;
            res.error_for_status()?;
        }
        
        Ok(())
    }

    pub async fn append_message(&self, session_key: &str, message: &Message) -> Result<()> {
        let valid_sheet_name = sanitize_sheet_name(session_key);
        // Ensure sheet exists first
        if let Err(e) = self.ensure_sheet_exists(session_key).await {
            error!("Failed to ensure sheet exists: {}", e);
        }

        let token = self.get_token().await?;
        
        let metadata_json = serde_json::to_string(&message.metadata).unwrap_or_default();
        
        let row = vec![
            json!(format!("{:?}", message.role).to_uppercase()),
            json!(message.content),
            json!(metadata_json),
        ];

        let body = json!({
            "values": [row]
        });

        let range = format!("{}!A:C", valid_sheet_name);
        let url = format!("https://sheets.googleapis.com/v4/spreadsheets/{}/values/{}:append?valueInputOption=RAW", self.spreadsheet_id, range);

        let res: Response = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&body)
            .send()
            .await?;

        if !res.status().is_success() {
             let text = match res.text().await {
                 Ok(t) => t,
                 Err(_) => String::new(),
             };
             return Err(anyhow!("Failed to append row: {}", text));
        }

        Ok(())
    }
}

fn sanitize_sheet_name(key: &str) -> String {
    key.replace(":", "_")
       .replace("/", "_")
       .replace("\\", "_")
       .replace("*", "_")
       .replace("?", "_")
       .replace("[", "(")
       .replace("]", ")")
       .chars()
       .take(100)
       .collect()
}
