pub enum PromptFormat {
    ChatML,
    Llama3,
    Standard,
}

pub struct PromptBuilder {
    format: PromptFormat,
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self {
            format: PromptFormat::Standard,
        }
    }
}

impl PromptBuilder {
    pub fn new(format: PromptFormat) -> Self {
        Self { format }
    }

    pub fn format_turn(&self, role: &str, content: &str) -> String {
        match self.format {
            PromptFormat::ChatML => format!("<|im_start|>{role}\n{content}<|im_end|>\n"),
            PromptFormat::Llama3 => {
                format!("<|start_header_id|>{role}<|end_header_id|>\n\n{content}<|eot_id|>")
            }
            PromptFormat::Standard => format!("{role}: {content}\n"),
        }
    }

    pub fn append_rag_context(&self, prompt: &str, citations: &[(&str, &str)]) -> String {
        if citations.is_empty() {
            return prompt.to_string();
        }
        let mut context_block = String::from("\nContext from library:\n");
        for (i, (source, text)) in citations.iter().enumerate() {
            context_block.push_str(&format!("[{}] (Source: {}):\n{}\n\n", i + 1, source, text));
        }
        format!("{}\n{}", context_block, prompt)
    }
}
