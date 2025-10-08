use crate::error::HttpError;
use crate::dtos::LLMReqeustTextInput;


#[derive(Clone)]
pub struct HttpClient {
    pub conn: reqwest::Client,
}

impl HttpClient {
    pub fn new(conn: reqwest::Client) -> Self {
        Self { conn }
    }

    pub async fn get_summary(
        &self, 
        llm_url: &str,
        model_name: &str,
        raw_text: &str,
    ) -> Result<String, HttpError> {
        let full_url = format!("{}/v1/responses", llm_url);
        let request_body = LLMReqeustTextInput{
            model: model_name.to_string(),
            input: format!("Summarize the following text in exactly 3 sentences. 
                The summary must be under 100 words in total. 
                Focus only on the main ideas, not details or examples. {}", raw_text),
        };

        let response = self.conn.post(full_url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| HttpError::server_error(e.to_string()))?;

        let json_value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| HttpError::server_error(e.to_string()))?;

        let llm_response_text = json_value["output"][0]["content"][0]["text"]
            .as_str() // 문자열로 변환
            .map(|s| s.to_string()) // String으로 복사
            .ok_or_else(|| {
                HttpError::server_error("Could not find text in response".to_string())
            })?;
        let summary: String;
        if let Some((_before, after)) = llm_response_text.split_once("</think>") {
            summary = after.trim().to_string();
        }
        else {
            return Err(HttpError::server_error("LLM parsing error".to_string()));
        }

        Ok(summary)
    }
}

