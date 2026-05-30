use crate::app::*;
use gpui::*;
use std::ops::Range;

impl ApiClientApp {
    pub(crate) fn toggle_find(
        &mut self,
        _: &ToggleFind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.find_state.is_some() {
            self.close_find_from_action(&CloseFind, window, cx);
        } else {
            // Retrieve selected text from the active response body CodeInput if available
            let initial_text = if let Some(active_req_id) = self.active_request_id {
                let effective_mode = if let Some(resp) = self
                    .workspace
                    .request_by_id(active_req_id)
                    .and_then(|r| r.response.as_ref())
                {
                    if resp.body.len() > super::LARGE_RESPONSE_WARNING_BYTES
                        && self.body_view_mode == BodyViewMode::Pretty
                    {
                        BodyViewMode::Raw
                    } else {
                        self.body_view_mode
                    }
                } else {
                    self.body_view_mode
                };
                if let Some(input) = self
                    .response_body_inputs
                    .get(&(active_req_id, effective_mode))
                {
                    let input_ref = input.read(cx);
                    let sel = input_ref.selected_range.clone();
                    if !sel.is_empty() {
                        input_ref.value()[sel].to_string()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            let query_input = cx.new(|cx| {
                TextInput::new_with_selector(
                    cx,
                    initial_text,
                    "Find in response...",
                    "FindQueryInput",
                )
            });

            // Subscribe to changes in the search query input
            cx.subscribe(&query_input, |this, _input_entity, _, cx| {
                this.update_search(cx);
            })
            .detach();

            self.find_state = Some(FindState {
                query_input,
                matches: Vec::new(),
                active_match_idx: None,
            });

            // Focus the find query input
            if let Some(find_state) = &self.find_state {
                let focus_handle = find_state.query_input.read(cx).focus_handle(cx);
                window.focus(&focus_handle);

                // Trigger initial search if there's pre-filled text
                self.update_search(cx);
            }

            cx.notify();
        }
    }

    pub(crate) fn find_next_from_action(
        &mut self,
        _: &FindNext,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(find_state) = &mut self.find_state else {
            return;
        };
        if find_state.matches.is_empty() {
            return;
        }

        let next_idx = match find_state.active_match_idx {
            Some(idx) => (idx + 1) % find_state.matches.len(),
            None => 0,
        };

        find_state.active_match_idx = Some(next_idx);
        let active_match = find_state.matches[next_idx].clone();

        // Push active match highlight to CodeInput
        self.update_code_input_highlights(active_match.clone(), cx);
        self.scroll_active_match_into_view(active_match.start, cx);
        cx.notify();
    }

    pub(crate) fn find_previous_from_action(
        &mut self,
        _: &FindPrevious,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(find_state) = &mut self.find_state else {
            return;
        };
        if find_state.matches.is_empty() {
            return;
        }

        let prev_idx = match find_state.active_match_idx {
            Some(idx) => {
                if idx == 0 {
                    find_state.matches.len() - 1
                } else {
                    idx - 1
                }
            }
            None => 0,
        };

        find_state.active_match_idx = Some(prev_idx);
        let active_match = find_state.matches[prev_idx].clone();

        // Push active match highlight to CodeInput
        self.update_code_input_highlights(active_match.clone(), cx);
        self.scroll_active_match_into_view(active_match.start, cx);
        cx.notify();
    }

    pub(crate) fn close_find_from_action(
        &mut self,
        _: &CloseFind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.find_state = None;

        // Clear CodeInput highlights
        if let Some(active_req_id) = self.active_request_id {
            let effective_mode = if let Some(resp) = self
                .workspace
                .request_by_id(active_req_id)
                .and_then(|r| r.response.as_ref())
            {
                if resp.body.len() > super::LARGE_RESPONSE_WARNING_BYTES
                    && self.body_view_mode == BodyViewMode::Pretty
                {
                    BodyViewMode::Raw
                } else {
                    self.body_view_mode
                }
            } else {
                self.body_view_mode
            };
            if let Some(input) = self
                .response_body_inputs
                .get(&(active_req_id, effective_mode))
            {
                input.update(cx, |input_ref, cx| {
                    input_ref.set_search_highlights(Vec::new(), None, cx);
                });

                // Return focus to the response editor FocusHandle
                let focus_handle = input.read(cx).focus_handle(cx);
                window.focus(&focus_handle);
            }
        }

        cx.notify();
    }

    pub(crate) fn update_search(&mut self, cx: &mut Context<Self>) {
        let Some(active_req_id) = self.active_request_id else {
            return;
        };
        let resp = self
            .workspace
            .request_by_id(active_req_id)
            .and_then(|r| r.response.as_ref())
            .cloned();
        let Some(resp) = resp else {
            return;
        };
        let body_content = self.formatted_body_for(active_req_id, &resp);

        let effective_mode = if resp.body.len() > super::LARGE_RESPONSE_WARNING_BYTES
            && self.body_view_mode == BodyViewMode::Pretty
        {
            BodyViewMode::Raw
        } else {
            self.body_view_mode
        };
        let Some(input) = self
            .response_body_inputs
            .get(&(active_req_id, effective_mode))
        else {
            return;
        };
        let find_state = self.find_state.as_mut().unwrap();
        let query = find_state.query_input.read(cx).value();

        if query.is_empty() {
            find_state.matches.clear();
            find_state.active_match_idx = None;
            input.update(cx, |input_ref, cx| {
                input_ref.set_search_highlights(Vec::new(), None, cx);
            });
            cx.notify();
            return;
        }

        // Perform case-insensitive substring search
        let body_lower = body_content.to_lowercase();
        let query_lower = query.to_lowercase();
        let matches: Vec<Range<usize>> = body_lower
            .match_indices(&query_lower)
            .map(|(start, _)| start..(start + query.len()))
            .collect();

        find_state.matches = matches;

        if !find_state.matches.is_empty() {
            find_state.active_match_idx = Some(0);
            let active_match = find_state.matches[0].clone();
            self.update_code_input_highlights(active_match.clone(), cx);
            self.scroll_active_match_into_view(active_match.start, cx);
        } else {
            find_state.active_match_idx = None;
            input.update(cx, |input_ref, cx| {
                input_ref.set_search_highlights(Vec::new(), None, cx);
            });
        }
        cx.notify();
    }

    fn update_code_input_highlights(&self, active_match: Range<usize>, cx: &mut Context<Self>) {
        let Some(active_req_id) = self.active_request_id else {
            return;
        };
        let Some(resp) = self
            .workspace
            .request_by_id(active_req_id)
            .and_then(|r| r.response.as_ref())
        else {
            return;
        };
        let effective_mode = if resp.body.len() > super::LARGE_RESPONSE_WARNING_BYTES
            && self.body_view_mode == BodyViewMode::Pretty
        {
            BodyViewMode::Raw
        } else {
            self.body_view_mode
        };
        let Some(input) = self
            .response_body_inputs
            .get(&(active_req_id, effective_mode))
        else {
            return;
        };
        let Some(find_state) = &self.find_state else {
            return;
        };

        input.update(cx, |input_ref, cx| {
            input_ref.set_search_highlights(find_state.matches.clone(), Some(active_match), cx);
        });
    }

    fn scroll_active_match_into_view(&self, offset: usize, cx: &mut Context<Self>) {
        let Some(active_req_id) = self.active_request_id else {
            return;
        };
        let Some(resp) = self
            .workspace
            .request_by_id(active_req_id)
            .and_then(|r| r.response.as_ref())
        else {
            return;
        };
        let effective_mode = if resp.body.len() > super::LARGE_RESPONSE_WARNING_BYTES
            && self.body_view_mode == BodyViewMode::Pretty
        {
            BodyViewMode::Raw
        } else {
            self.body_view_mode
        };
        let Some(input) = self
            .response_body_inputs
            .get(&(active_req_id, effective_mode))
        else {
            return;
        };

        let row = input.read(cx).line_index.row_for_offset(offset);
        let line_height = px(20.0); // Standard line height used in rendering
        let y_offset = line_height * row as f32;

        self.response_scroll_handle
            .set_offset(point(self.response_scroll_handle.offset().x, -y_offset));
    }

    pub(crate) fn focus_next(&mut self, _: &FocusNext, window: &mut Window, _: &mut Context<Self>) {
        window.focus_next();
    }

    pub(crate) fn focus_prev(&mut self, _: &FocusPrev, window: &mut Window, _: &mut Context<Self>) {
        window.focus_prev();
    }
}
