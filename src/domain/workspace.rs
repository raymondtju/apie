use std::time::Duration;

use super::preview::run_preview;
use super::{
    ClientError, CollectionItem, Environment, Header, Request, ResponseRecord, Result, Workspace,
};
use crate::http::send_http_request;

impl Workspace {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            active_environment: "development".to_string(),
            environments: vec![Environment {
                name: "development".to_string(),
                variables: vec![Header::new("base_url", "https://api.example.com")],
            }],
            items: Vec::new(),
            expanded_folders: Vec::new(),
        }
    }

    pub fn create_request(&mut self, request: Request) {
        self.items.push(CollectionItem::Request(request));
    }

    pub fn requests(&self) -> Vec<&Request> {
        let mut requests = Vec::new();
        collect_requests(&self.items, &mut requests);
        requests
    }

    pub fn edit_request(&mut self, id: &str, update: impl FnOnce(&mut Request)) -> Result<()> {
        let request = find_request_mut(&mut self.items, id)
            .ok_or_else(|| ClientError::MissingRequest(id.to_string()))?;
        update(request);
        Ok(())
    }

    pub fn send_preview(&mut self, id: &str) -> Result<ResponseRecord> {
        let request = find_request_mut(&mut self.items, id)
            .ok_or_else(|| ClientError::MissingRequest(id.to_string()))?;
        let response = run_preview(request, Duration::from_millis(42));
        request.history.insert(0, response.clone());
        Ok(response)
    }

    pub fn send_http(&mut self, id: &str) -> Result<ResponseRecord> {
        let request = find_request_mut(&mut self.items, id)
            .ok_or_else(|| ClientError::MissingRequest(id.to_string()))?;
        let response = send_http_request(request)?;
        request.history.insert(0, response.clone());
        Ok(response)
    }
}
fn collect_requests<'a>(items: &'a [CollectionItem], out: &mut Vec<&'a Request>) {
    for item in items {
        match item {
            CollectionItem::Request(request) => out.push(request),
            CollectionItem::Folder { items, .. } => collect_requests(items, out),
        }
    }
}

fn find_request_mut<'a>(items: &'a mut [CollectionItem], id: &str) -> Option<&'a mut Request> {
    for item in items {
        match item {
            CollectionItem::Request(request) if request.id == id => return Some(request),
            CollectionItem::Folder { items, .. } => {
                if let Some(request) = find_request_mut(items, id) {
                    return Some(request);
                }
            }
            _ => {}
        }
    }
    None
}
