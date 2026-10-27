use std::fmt;

use crate::permission::Permission;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl RpcError {
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL: i64 = -32603;
    pub const FORBIDDEN: i64 = -32001;
    pub const UNAVAILABLE: i64 = -32002;

    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(Self::INVALID_PARAMS, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(Self::FORBIDDEN, message)
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(Self::UNAVAILABLE, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(Self::INTERNAL, message)
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(Self::METHOD_NOT_FOUND, format!("método desconhecido: {method}"))
    }

    pub fn required(key: &str) -> Self {
        Self::invalid(format!("\"{key}\" é obrigatório"))
    }

    pub fn missing_permission(p: Permission) -> Self {
        Self::forbidden(format!("o manifesto não declara a permissão \"{}\"", p.id()))
    }

    pub fn window_not_ready() -> Self {
        Self::unavailable("a janela ainda não está pronta")
    }

    pub fn no_window() -> Self {
        Self::unavailable("a janela não tem onde perguntar")
    }

    pub fn no_tab(id: &str) -> Self {
        Self::invalid(format!("não há painel \"{id}\""))
    }

    pub fn no_focused_tab() -> Self {
        Self::invalid("nenhum painel em foco")
    }

    pub fn no_session_on_screen() -> Self {
        Self::invalid("nenhuma sessão na tela")
    }

    pub fn no_process(title: &str) -> Self {
        Self::invalid(format!("\"{title}\" não tem processo pra receber texto"))
    }

    pub fn not_owner() -> Self {
        Self::forbidden("só dá pra fechar um terminal que o plugin abriu com \"owned\"")
    }

    pub fn exited() -> Self {
        Self::unavailable("o plugin saiu")
    }

    pub fn not_running() -> Self {
        Self::unavailable("o plugin não está rodando")
    }

    pub fn not_enabled() -> Self {
        Self::unavailable("o plugin não está ligado")
    }

    pub fn timeout(method: &str, seconds: u64) -> Self {
        Self::unavailable(format!("\"{method}\" ficou sem resposta por {seconds}s"))
    }
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RpcError {}
