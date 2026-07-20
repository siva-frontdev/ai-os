use async_trait::async_trait;
use execution_core::OutputParser;

#[derive(Debug)]
pub struct DefaultOutputParser {
    parsers: Vec<Box<dyn OutputParser>>,
}

impl DefaultOutputParser {
    pub fn new(parsers: Vec<Box<dyn OutputParser>>) -> Self {
        Self { parsers }
    }
}

impl Default for DefaultOutputParser {
    fn default() -> Self {
        Self {
            parsers: vec![Box::new(JsonParser), Box::new(RawParser)],
        }
    }
}

#[async_trait]
impl OutputParser for DefaultOutputParser {
    async fn parse(
        &self,
        data: &[u8],
    ) -> execution_core::ExecutionResult<Option<serde_json::Value>> {
        for parser in &self.parsers {
            if let Ok(Some(value)) = parser.parse(data).await {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    fn name(&self) -> &'static str {
        "default"
    }

    fn mime_types(&self) -> Vec<&'static str> {
        vec!["*/*"]
    }
}

#[derive(Debug)]
pub struct JsonParser;

#[async_trait]
impl OutputParser for JsonParser {
    async fn parse(
        &self,
        data: &[u8],
    ) -> execution_core::ExecutionResult<Option<serde_json::Value>> {
        match serde_json::from_slice(data) {
            Ok(value) => Ok(Some(value)),
            Err(_) => Ok(None),
        }
    }

    fn name(&self) -> &'static str {
        "json"
    }

    fn mime_types(&self) -> Vec<&'static str> {
        vec!["application/json"]
    }
}

#[derive(Debug)]
pub struct RawParser;

#[async_trait]
impl OutputParser for RawParser {
    async fn parse(
        &self,
        data: &[u8],
    ) -> execution_core::ExecutionResult<Option<serde_json::Value>> {
        let s = String::from_utf8_lossy(data);
        Ok(Some(serde_json::Value::String(s.into_owned())))
    }

    fn name(&self) -> &'static str {
        "raw"
    }

    fn mime_types(&self) -> Vec<&'static str> {
        vec!["*/*"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_json_parser_valid() {
        let parser = JsonParser;
        let data = b"{\"key\": \"value\"}";
        let result = parser.parse(data).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap()["key"], "value");
    }

    #[tokio::test]
    async fn test_json_parser_invalid() {
        let parser = JsonParser;
        let data = b"not json";
        let result = parser.parse(data).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_json_parser_empty() {
        let parser = JsonParser;
        let data = b"";
        let result = parser.parse(data).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_raw_parser() {
        let parser = RawParser;
        let data = b"hello world";
        let result = parser.parse(data).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "hello world");
    }

    #[tokio::test]
    async fn test_raw_parser_empty() {
        let parser = RawParser;
        let data = b"";
        let result = parser.parse(data).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "");
    }

    #[tokio::test]
    async fn test_default_parser_json_first() {
        let parser = DefaultOutputParser::default();
        let data = b"{\"a\": 1}";
        let result = parser.parse(data).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap()["a"], 1);
    }

    #[tokio::test]
    async fn test_default_parser_fallback_to_raw() {
        let parser = DefaultOutputParser::default();
        let data = b"plain text";
        let result = parser.parse(data).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "plain text");
    }
}
