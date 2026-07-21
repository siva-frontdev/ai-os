use super::error::*;
use async_trait::async_trait;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use intelligence_core::error::ModelResult;
use intelligence_core::traits::StreamDecoder;
use intelligence_core::types::{StreamChunk, StreamHandler};
use std::collections::HashMap;
use std::pin::Pin;

#[derive(Debug, Default)]
pub struct DefaultStreamingManager;

#[async_trait]
impl StreamDecoder for DefaultStreamingManager {
    async fn decode(
        &self,
        stream: Pin<Box<dyn Stream<Item = Bytes> + Send>>,
    ) -> ModelResult<StreamHandler> {
        use futures::StreamExt;
        let chunks: Vec<Bytes> = stream.collect().await;
        let mut handler = StreamHandler {
            request_id: intelligence_core::types::RequestId::new(),
            buffer: vec![],
            tool_call_buffers: HashMap::new(),
            safety_events: vec![],
            usage: Default::default(),
            finished: false,
        };
        for chunk in chunks {
            handler.buffer.push(StreamChunk::ContentDelta(
                String::from_utf8_lossy(&chunk).into(),
            ));
        }
        handler.finished = true;
        Ok(handler)
    }

    fn aggregate_chunks(
        &self,
        handler: StreamHandler,
    ) -> ModelResult<intelligence_core::types::ModelResponse> {
        let content = handler
            .buffer
            .iter()
            .filter_map(|c| match c {
                StreamChunk::ContentDelta(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        Ok(intelligence_core::types::ModelResponse {
            request_id: handler.request_id,
            model_id: intelligence_core::types::ModelId::new(),
            content,
            usage: handler.usage,
            finished: true,
            finish_reason: Some("stop".into()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    #[tokio::test]
    async fn decode_stream() {
        let mgr = DefaultStreamingManager;
        let s: Pin<Box<dyn Stream<Item = Bytes> + Send>> = Box::pin(stream::iter(vec![
            Bytes::from_static(b"hi"),
            Bytes::from_static(b" there"),
        ]));
        let handler = mgr.decode(s).await.unwrap();
        assert!(handler.finished);
    }
}
