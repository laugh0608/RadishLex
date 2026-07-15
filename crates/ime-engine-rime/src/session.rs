use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::mem;
use std::ptr;
use std::rc::Rc;
use std::slice;
use std::sync::Arc;

use radishlex_ime_core::{
    Candidate, Commit, CommitSource, Composition, CoreError, CoreResult, Engine, KeyEvent,
    KeyOutcome, SchemaId,
};

use crate::config::RimeEngineConfig;
use crate::convert::{candidate_from_view, composition_from_parts, RimeCandidateView};
use crate::error::{RimeEngineError, RimeEngineResult};
use crate::ffi::{self, RimeApi, RimeCommit, RimeContext, RimeSessionId, TRUE};
use crate::keymap::{classify_key_event, rime_keycode, RimeKeyInput, RimeNamedKey};
use crate::runtime::{
    current_schema, ensure_true, require_api_function, select_schema_exact, RimeRuntime,
};

#[derive(Debug)]
pub struct RimeEngine {
    config: RimeEngineConfig,
    runtime: Arc<RimeRuntime>,
    session_id: RimeSessionId,
    _owner_thread_only: PhantomData<Rc<()>>,
}

impl RimeEngine {
    pub fn new(config: RimeEngineConfig) -> RimeEngineResult<Self> {
        let runtime = RimeRuntime::process()?;
        Self::new_with_runtime(config, runtime)
    }

    pub(crate) fn new_with_runtime(
        config: RimeEngineConfig,
        runtime: Arc<RimeRuntime>,
    ) -> RimeEngineResult<Self> {
        let session_id = runtime.create_session(&config)?;
        Ok(Self {
            config,
            runtime,
            session_id,
            _owner_thread_only: PhantomData,
        })
    }

    pub fn config(&self) -> &RimeEngineConfig {
        &self.config
    }

    pub fn session_id(&self) -> RimeSessionId {
        self.session_id
    }

    fn highlighted_candidate_index(&self) -> RimeEngineResult<Option<usize>> {
        self.runtime.with_api(|api| {
            Self::read_context(api, self.session_id, "get_context", |context| {
                if context.menu.num_candidates <= 0 {
                    return Ok(None);
                }
                let index = context.menu.highlighted_candidate_index;
                if index < 0 || index >= context.menu.num_candidates {
                    return Err(RimeEngineError::FfiFailure {
                        stage: "get_context",
                        message: format!(
                            "highlighted candidate index {index} is outside current page length {}",
                            context.menu.num_candidates
                        ),
                    });
                }
                Ok(Some(index as usize))
            })
        })
    }

    fn read_commit(api: &RimeApi, session_id: RimeSessionId) -> RimeEngineResult<Option<Commit>> {
        // SAFETY: api belongs to the locked process runtime and session_id is
        // owned by this RimeEngine. RimeCommit is
        // initialized with the self-versioned data_size field expected by librime.
        unsafe {
            let get_commit = require_api_function(api.get_commit, "get_commit")?;
            let free_commit = require_api_function(api.free_commit, "free_commit")?;
            let mut commit = rime_commit();
            if get_commit(session_id, &mut commit) != TRUE {
                return Ok(None);
            }

            let text = c_string_field("commit.text", commit.text);
            let freed = free_commit(&mut commit);
            ensure_true("free_commit", freed)?;

            Ok(Some(Commit::new(text?, CommitSource::Engine)))
        }
    }

    fn read_context<T>(
        api: &RimeApi,
        session_id: RimeSessionId,
        stage: &'static str,
        read: impl FnOnce(&RimeContext) -> RimeEngineResult<T>,
    ) -> RimeEngineResult<T> {
        // SAFETY: api belongs to the locked process runtime and session_id is
        // owned by this RimeEngine. RimeContext is
        // initialized with the self-versioned data_size field expected by librime.
        unsafe {
            let get_context = require_api_function(api.get_context, "get_context")?;
            let free_context = require_api_function(api.free_context, "free_context")?;
            let mut context = rime_context();
            let got_context = get_context(session_id, &mut context);
            ensure_true(stage, got_context)?;

            let result = read(&context);
            let freed = free_context(&mut context);
            ensure_true("free_context", freed)?;
            result
        }
    }
}

impl Engine for RimeEngine {
    fn reset(&mut self) -> CoreResult<()> {
        self.runtime
            .with_api(|api| {
                // SAFETY: runtime serializes the native call, session_id belongs
                // to this engine, and the function is checked before use.
                unsafe {
                    let clear_composition =
                        require_api_function(api.clear_composition, "clear_composition")?;
                    clear_composition(self.session_id);
                }
                Ok(())
            })
            .map_err(rime_to_core)
    }

    fn push_key(&mut self, key: KeyEvent) -> CoreResult<KeyOutcome> {
        let input = classify_key_event(key);
        if input == RimeKeyInput::Named(RimeNamedKey::Space) {
            if let Some(index) = self.highlighted_candidate_index().map_err(rime_to_core)? {
                return self.select_candidate(index);
            }
        }
        let Some(keycode) = rime_keycode(input) else {
            return Ok(KeyOutcome::ignored());
        };

        self.runtime
            .with_api(|api| {
                // SAFETY: runtime serializes the native call, session_id belongs
                // to this engine, and the function is checked before use.
                let consumed = unsafe {
                    let process_key = require_api_function(api.process_key, "process_key")?;
                    process_key(self.session_id, keycode, 0)
                };
                if consumed != TRUE {
                    return Ok(KeyOutcome::ignored());
                }

                let commit = Self::read_commit(api, self.session_id)?;
                Ok(KeyOutcome::new(true, commit))
            })
            .map_err(rime_to_core)
    }

    fn composition(&self) -> CoreResult<Composition> {
        self.runtime
            .with_api(|api| {
                Self::read_context(api, self.session_id, "get_context", |context| {
                    let preedit = if context.composition.preedit.is_null() {
                        String::new()
                    } else {
                        // SAFETY: librime owns this null-terminated string until
                        // free_context.
                        unsafe {
                            c_string_field("composition.preedit", context.composition.preedit)?
                        }
                    };
                    composition_from_parts(&preedit, context.composition.cursor_pos)
                })
            })
            .map_err(rime_to_core)
    }

    fn candidates(&self) -> CoreResult<Vec<Candidate>> {
        self.runtime
            .with_api(|api| {
                Self::read_context(api, self.session_id, "get_context", |context| {
                    if context.menu.num_candidates <= 0 || context.menu.candidates.is_null() {
                        return Ok(Vec::new());
                    }

                    // SAFETY: librime owns the candidate array until free_context.
                    // The length comes from the same RimeMenu structure.
                    let raw_candidates = unsafe {
                        slice::from_raw_parts(
                            context.menu.candidates,
                            context.menu.num_candidates as usize,
                        )
                    };

                    raw_candidates
                        .iter()
                        .map(|candidate| {
                            // SAFETY: candidate strings are owned by librime until
                            // free_context below.
                            let text = unsafe { c_string_field("candidate.text", candidate.text)? };
                            let annotation = unsafe { optional_c_string(candidate.comment)? };
                            candidate_from_view(RimeCandidateView {
                                text: &text,
                                reading: None,
                                annotation: annotation.as_deref(),
                            })
                        })
                        .collect()
                })
            })
            .map_err(rime_to_core)
    }

    fn input_code(&self) -> CoreResult<String> {
        self.runtime
            .with_api(|api| {
                // SAFETY: runtime serializes the native call, session_id belongs
                // to this engine, and librime owns the returned string.
                unsafe {
                    let get_input = require_api_function(api.get_input, "get_input")?;
                    c_string_field("input_code", get_input(self.session_id))
                }
            })
            .map_err(rime_to_core)
    }

    fn select_candidate(&mut self, index: usize) -> CoreResult<KeyOutcome> {
        self.runtime
            .with_api(|api| {
                // SAFETY: runtime serializes the native call, session_id belongs
                // to this engine, and the function is checked before use.
                let selected = unsafe {
                    let select_candidate = require_api_function(
                        api.select_candidate_on_current_page,
                        "select_candidate_on_current_page",
                    )?;
                    select_candidate(self.session_id, index)
                };
                if selected != TRUE {
                    return Err(RimeEngineError::FfiFailure {
                        stage: "select_candidate",
                        message: format!("candidate index {index} was not accepted by librime"),
                    });
                }

                Ok(KeyOutcome::new(
                    true,
                    Self::read_commit(api, self.session_id)?,
                ))
            })
            .map_err(rime_to_core)
    }

    fn set_schema(&mut self, schema: SchemaId) -> CoreResult<()> {
        let schema_cstring = CString::new(schema.as_str()).map_err(|error| {
            rime_to_core(RimeEngineError::EncodingFailure {
                field: "schema",
                message: error.to_string(),
            })
        })?;

        self.runtime
            .with_api(|api| select_schema_exact(api, self.session_id, schema_cstring.as_c_str()))
            .map_err(rime_to_core)?;

        self.config.replace_schema(schema);
        Ok(())
    }

    fn schema(&self) -> CoreResult<SchemaId> {
        let schema = self
            .runtime
            .with_api(|api| current_schema(api, self.session_id))
            .map_err(rime_to_core)?;
        SchemaId::new(schema)
    }
}

impl Drop for RimeEngine {
    fn drop(&mut self) {
        self.runtime.release_session(self.session_id);
        self.session_id = 0;
    }
}

fn rime_commit() -> RimeCommit {
    RimeCommit {
        data_size: (mem::size_of::<RimeCommit>() - mem::size_of::<i32>()) as i32,
        text: ptr::null_mut(),
    }
}

fn rime_context() -> RimeContext {
    RimeContext {
        data_size: (mem::size_of::<RimeContext>() - mem::size_of::<i32>()) as i32,
        composition: ffi::RimeComposition {
            length: 0,
            cursor_pos: 0,
            sel_start: 0,
            sel_end: 0,
            preedit: ptr::null_mut(),
        },
        menu: ffi::RimeMenu {
            page_size: 0,
            page_no: 0,
            is_last_page: TRUE,
            highlighted_candidate_index: 0,
            num_candidates: 0,
            candidates: ptr::null_mut(),
            select_keys: ptr::null_mut(),
        },
        commit_text_preview: ptr::null_mut(),
        select_labels: ptr::null_mut(),
    }
}

unsafe fn c_string_field(field: &'static str, value: *const i8) -> RimeEngineResult<String> {
    if value.is_null() {
        return Err(RimeEngineError::EncodingFailure {
            field,
            message: "null C string".to_owned(),
        });
    }

    CStr::from_ptr(value)
        .to_str()
        .map(|value| value.to_owned())
        .map_err(|error| RimeEngineError::EncodingFailure {
            field,
            message: error.to_string(),
        })
}

unsafe fn optional_c_string(value: *const i8) -> RimeEngineResult<Option<String>> {
    if value.is_null() {
        Ok(None)
    } else {
        Ok(Some(c_string_field("candidate.comment", value)?))
    }
}

fn rime_to_core(error: RimeEngineError) -> CoreError {
    CoreError::engine(error.to_string())
}
