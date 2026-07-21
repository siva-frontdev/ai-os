use super::error::*;
use async_trait::async_trait;
use dashmap::DashMap;
use intelligence_core::error::ModelError;
use intelligence_core::traits::PromptRenderer;
use intelligence_core::types::{PromptId, PromptTemplate, RenderedPrompt};
use std::collections::HashMap;

fn to_model_result<T>(res: PromptsResult<T>) -> intelligence_core::error::ModelResult<T> {
    res.map_err(|e| ModelError::Io(e.to_string()))
}

#[derive(Debug, Default)]
pub struct DefaultPromptRenderer {
    templates: DashMap<String, PromptTemplate>,
    by_id: DashMap<String, PromptTemplate>,
}

impl DefaultPromptRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    fn _register(&self, template: PromptTemplate) -> PromptsResult<()> {
        let name = template.name.clone();
        let id_key = template.id.to_string();
        self.templates.insert(name, template.clone());
        self.by_id.insert(id_key, template);
        Ok(())
    }

    fn _get(&self, id: &str) -> PromptsResult<Option<PromptTemplate>> {
        Ok(self.by_id.get(id).map(|entry| entry.clone()))
    }

    fn render_sync(
        &self,
        template: &PromptTemplate,
        vars: &HashMap<String, String>,
    ) -> PromptsResult<RenderedPrompt> {
        let mut content = template.content.clone();
        for (k, v) in vars {
            let placeholder = format!("{{{{{}}}}}", k);
            content = content.replace(&placeholder, v);
        }
        let token_count = content.split_whitespace().count();
        Ok(RenderedPrompt {
            content,
            token_count,
        })
    }
}

#[async_trait]
impl PromptRenderer for DefaultPromptRenderer {
    async fn register_template(
        &self,
        template: PromptTemplate,
    ) -> intelligence_core::error::ModelResult<()> {
        to_model_result(self._register(template))
    }

    async fn get_template(
        &self,
        template_id: &PromptId,
    ) -> intelligence_core::error::ModelResult<Option<PromptTemplate>> {
        to_model_result(self._get(&template_id.to_string()))
    }

    async fn list_templates(
        &self,
        _tags: &[String],
    ) -> intelligence_core::error::ModelResult<Vec<PromptTemplate>> {
        Ok(self.templates.iter().map(|e| e.clone()).collect())
    }

    async fn render(
        &self,
        template: &PromptTemplate,
        variables: &HashMap<String, String>,
    ) -> intelligence_core::error::ModelResult<RenderedPrompt> {
        to_model_result(self.render_sync(template, variables))
    }

    async fn render_with_context(
        &self,
        template: &PromptTemplate,
        variables: &HashMap<String, String>,
        _context: &intelligence_core::types::ContextWindow,
    ) -> intelligence_core::error::ModelResult<RenderedPrompt> {
        to_model_result(self.render_sync(template, variables))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn render_substitutes_variables() {
        let renderer = DefaultPromptRenderer::new();
        let tpl = PromptTemplate {
            id: PromptId::new(),
            name: "greeting".into(),
            content: "Hello, {{name}}!".into(),
            tags: vec![],
        };
        let mut vars = HashMap::new();
        vars.insert("name".into(), "World".into());
        let rendered = renderer.render_sync(&tpl, &vars).unwrap();
        assert_eq!(rendered.content, "Hello, World!");
    }
}
