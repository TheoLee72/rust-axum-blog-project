use crate::error::HttpError;
use crate::embed::embed_service_client::EmbedServiceClient;
use crate::embed::EmbedRequest;
#[derive(Clone)]
pub struct GRPCClient {
    pub embed_client: EmbedServiceClient<tonic::transport::Channel>,
}

impl GRPCClient {
    pub fn new(embed_client: EmbedServiceClient<tonic::transport::Channel>) -> Self {
        Self { embed_client }
    }

    pub async fn get_embedding_docs(
        &self, //GRPCClient를 mutable reference로 받아올지를 결정할 뿐. rust에서 struct내부 각 요소를 mutable하게 설정할지 말지 결정하는 건 없음. 전체 struct단위 따라감. 
        raw_text: &str, //그럼 GRPCClient는 AppState따라감. 처음부터 mutable아니었으니까 mutable reference 불가. mutable로 불러온다음. 내부에서 다시 mutable에 bind시켜야함. 
        title: &str,
    ) -> Result<Vec<f32>, HttpError> {
        let request = tonic::Request::new(EmbedRequest {
            text: raw_text.to_string(),
            task: format!("title: {} | text", title),
        });
        let mut client = self.embed_client.clone(); //여기가 그 부분.
        let response = client.embed_query(request)
            .await
            .map_err(|e| HttpError::server_error(e.to_string()))?
            .into_inner();

        let embedding = response.embedding;
        Ok(embedding)

    }

    pub async fn get_embedding_query(
        &self,
        q: &str,
    ) -> Result<Vec<f32>, HttpError> {
        let request = tonic::Request::new(EmbedRequest {
            text: q.to_string(),
            task: "task: search result | query".to_string(),
        });
        let mut client = self.embed_client.clone();
        let response = client.embed_query(request)
            .await
            .map_err(|e| HttpError::server_error(e.to_string()))?
            .into_inner();

        let embedding = response.embedding;
        Ok(embedding)

    }
}

