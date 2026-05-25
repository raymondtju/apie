use std::time::Duration;

use super::preview::run_preview;
use super::{
    ClientError, Collection, CollectionItem, Environment, Header, Request, ResponseRecord, Result,
    Workspace,
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
            expanded_collections: Vec::new(),
        }
    }

    pub fn create_request_in_collection(
        &mut self,
        collection_id: &str,
        request: Request,
    ) -> Result<()> {
        let collection = self
            .items
            .iter_mut()
            .find(|c| c.id == collection_id)
            .ok_or_else(|| ClientError::Import("Collection not found".into()))?;
        collection.items.push(CollectionItem::Request(request));
        Ok(())
    }

    /// Add a request to the first available collection, or create one.
    pub fn create_request(&mut self, request: Request) {
        if let Some(collection) = self.items.first_mut() {
            collection.items.push(CollectionItem::Request(request));
        } else {
            let id = self.items.len().to_string();
            self.items.push(Collection {
                id: id.clone(),
                name: "Default".into(),
                items: vec![CollectionItem::Request(request)],
            });
        }
    }

    pub fn requests(&self) -> Vec<&Request> {
        let mut requests = Vec::new();
        for collection in &self.items {
            collect_requests(&collection.items, &mut requests);
        }
        requests
    }

    pub fn edit_request(&mut self, id: &str, update: impl FnOnce(&mut Request)) -> Result<()> {
        for collection in &mut self.items {
            if let Some(request) = find_request_mut(&mut collection.items, id) {
                update(request);
                return Ok(());
            }
        }
        Err(ClientError::MissingRequest(id.to_string()))
    }

    pub fn send_preview(&mut self, id: &str) -> Result<ResponseRecord> {
        for collection in &mut self.items {
            if let Some(request) = find_request_mut(&mut collection.items, id) {
                let response = run_preview(request, Duration::from_millis(42));
                request.history.insert(0, response.clone());
                return Ok(response);
            }
        }
        Err(ClientError::MissingRequest(id.to_string()))
    }

    pub fn send_http(&mut self, id: &str) -> Result<ResponseRecord> {
        for collection in &mut self.items {
            if let Some(request) = find_request_mut(&mut collection.items, id) {
                let response = send_http_request(request)?;
                request.history.insert(0, response.clone());
                return Ok(response);
            }
        }
        Err(ClientError::MissingRequest(id.to_string()))
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
