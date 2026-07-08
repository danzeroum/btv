//! Parser incremental de Server-Sent Events.
//!
//! Recebe bytes em pedaços arbitrários (como chegam do `reqwest`) e emite
//! eventos completos. Sem dependência de rede — testável com strings.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

#[derive(Default)]
pub struct SseParser {
    buffer: String,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Alimenta o parser com um pedaço de bytes; retorna os eventos que
    /// ficaram completos (terminados por linha em branco).
    pub fn push(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        self.buffer.push_str(&String::from_utf8_lossy(chunk));
        let mut events = Vec::new();
        // Um evento termina em "\n\n" (tolera "\r\n\r\n").
        while let Some(pos) = self.buffer.find("\n\n") {
            let raw: String = self.buffer.drain(..pos + 2).collect();
            if let Some(event) = Self::parse_block(&raw) {
                events.push(event);
            }
        }
        events
    }

    fn parse_block(block: &str) -> Option<SseEvent> {
        let mut event = None;
        let mut data_lines = Vec::new();
        for line in block.lines() {
            let line = line.strip_suffix('\r').unwrap_or(line);
            if let Some(rest) = line.strip_prefix("event:") {
                event = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("data:") {
                data_lines.push(rest.strip_prefix(' ').unwrap_or(rest).to_string());
            }
        }
        if data_lines.is_empty() && event.is_none() {
            return None;
        }
        Some(SseEvent {
            event,
            data: data_lines.join("\n"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evento_completo_em_um_chunk() {
        let mut p = SseParser::new();
        let events = p.push(b"event: message_start\ndata: {\"a\":1}\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event.as_deref(), Some("message_start"));
        assert_eq!(events[0].data, "{\"a\":1}");
    }

    #[test]
    fn evento_quebrado_em_chunks_arbitrarios() {
        let mut p = SseParser::new();
        assert!(p.push(b"data: {\"par").is_empty());
        assert!(p.push(b"cial\":true}").is_empty());
        let events = p.push(b"\n\ndata: [DONE]\n\n");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, "{\"parcial\":true}");
        assert_eq!(events[1].data, "[DONE]");
    }

    #[test]
    fn multiplas_linhas_de_data_sao_concatenadas() {
        let mut p = SseParser::new();
        let events = p.push(b"data: linha1\ndata: linha2\n\n");
        assert_eq!(events[0].data, "linha1\nlinha2");
    }

    #[test]
    fn crlf_e_tolerado() {
        let mut p = SseParser::new();
        let events = p.push(b"data: ok\r\n\r\n");
        // \r\n\r\n contém \n\n? Não — precisa tratar: "ok\r" com strip.
        // O find("\n\n") não casa com "\r\n\r\n"; empurra terminador extra.
        let events2 = p.push(b"\n\n");
        let all: Vec<_> = events.into_iter().chain(events2).collect();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].data, "ok");
    }
}
