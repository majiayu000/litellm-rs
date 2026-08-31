use super::{
    audio_speech, audio_transcriptions, audio_translations, cancel_response, chat_completions,
    create_moderation, create_response, delete_response, embeddings, get_model, get_response,
    image_edits, image_generations, image_variations, list_models, list_response_input_items,
    rerank,
};
use actix_web::{Route, web};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StableInferenceMethod {
    Delete,
    Get,
    Post,
}

impl StableInferenceMethod {
    #[cfg(test)]
    pub(super) const fn openapi_key(self) -> &'static str {
        match self {
            Self::Delete => "delete",
            Self::Get => "get",
            Self::Post => "post",
        }
    }

    fn actix_route(self) -> Route {
        match self {
            Self::Delete => web::delete(),
            Self::Get => web::get(),
            Self::Post => web::post(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StableInferenceOperation {
    AudioSpeech,
    AudioTranscription,
    AudioTranslation,
    CancelResponse,
    ChatCompletion,
    CreateResponse,
    DeleteResponse,
    Embedding,
    GetModel,
    GetResponse,
    ImageEdit,
    ImageGeneration,
    ImageVariation,
    ListModels,
    ListResponseInputItems,
    Moderation,
    Rerank,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StableInferenceRoute {
    pub(super) path: &'static str,
    pub(super) method: StableInferenceMethod,
    operation: StableInferenceOperation,
}

const STABLE_INFERENCE_ROUTES: &[StableInferenceRoute] = &[
    StableInferenceRoute {
        path: "/v1/chat/completions",
        method: StableInferenceMethod::Post,
        operation: StableInferenceOperation::ChatCompletion,
    },
    StableInferenceRoute {
        path: "/v1/responses",
        method: StableInferenceMethod::Post,
        operation: StableInferenceOperation::CreateResponse,
    },
    StableInferenceRoute {
        path: "/v1/responses/{response_id}",
        method: StableInferenceMethod::Get,
        operation: StableInferenceOperation::GetResponse,
    },
    StableInferenceRoute {
        path: "/v1/responses/{response_id}",
        method: StableInferenceMethod::Delete,
        operation: StableInferenceOperation::DeleteResponse,
    },
    StableInferenceRoute {
        path: "/v1/responses/{response_id}/cancel",
        method: StableInferenceMethod::Post,
        operation: StableInferenceOperation::CancelResponse,
    },
    StableInferenceRoute {
        path: "/v1/responses/{response_id}/input_items",
        method: StableInferenceMethod::Get,
        operation: StableInferenceOperation::ListResponseInputItems,
    },
    StableInferenceRoute {
        path: "/v1/embeddings",
        method: StableInferenceMethod::Post,
        operation: StableInferenceOperation::Embedding,
    },
    StableInferenceRoute {
        path: "/v1/images/generations",
        method: StableInferenceMethod::Post,
        operation: StableInferenceOperation::ImageGeneration,
    },
    StableInferenceRoute {
        path: "/v1/images/edits",
        method: StableInferenceMethod::Post,
        operation: StableInferenceOperation::ImageEdit,
    },
    StableInferenceRoute {
        path: "/v1/images/variations",
        method: StableInferenceMethod::Post,
        operation: StableInferenceOperation::ImageVariation,
    },
    StableInferenceRoute {
        path: "/v1/audio/speech",
        method: StableInferenceMethod::Post,
        operation: StableInferenceOperation::AudioSpeech,
    },
    StableInferenceRoute {
        path: "/v1/audio/transcriptions",
        method: StableInferenceMethod::Post,
        operation: StableInferenceOperation::AudioTranscription,
    },
    StableInferenceRoute {
        path: "/v1/audio/translations",
        method: StableInferenceMethod::Post,
        operation: StableInferenceOperation::AudioTranslation,
    },
    StableInferenceRoute {
        path: "/v1/moderations",
        method: StableInferenceMethod::Post,
        operation: StableInferenceOperation::Moderation,
    },
    StableInferenceRoute {
        path: "/v1/rerank",
        method: StableInferenceMethod::Post,
        operation: StableInferenceOperation::Rerank,
    },
    StableInferenceRoute {
        path: "/v1/models",
        method: StableInferenceMethod::Get,
        operation: StableInferenceOperation::ListModels,
    },
    StableInferenceRoute {
        path: "/v1/models/{model_id}",
        method: StableInferenceMethod::Get,
        operation: StableInferenceOperation::GetModel,
    },
];

pub(super) const fn stable_inference_routes() -> &'static [StableInferenceRoute] {
    STABLE_INFERENCE_ROUTES
}

pub(super) fn configure(cfg: &mut web::ServiceConfig) {
    for route in stable_inference_routes() {
        let path = route
            .path
            .strip_prefix("/v1")
            .expect("stable inference route must be under /v1");
        match route.operation {
            StableInferenceOperation::AudioSpeech => {
                cfg.route(path, route.method.actix_route().to(audio_speech));
            }
            StableInferenceOperation::AudioTranscription => {
                cfg.route(path, route.method.actix_route().to(audio_transcriptions));
            }
            StableInferenceOperation::AudioTranslation => {
                cfg.route(path, route.method.actix_route().to(audio_translations));
            }
            StableInferenceOperation::CancelResponse => {
                cfg.route(path, route.method.actix_route().to(cancel_response));
            }
            StableInferenceOperation::ChatCompletion => {
                cfg.route(path, route.method.actix_route().to(chat_completions));
            }
            StableInferenceOperation::CreateResponse => {
                cfg.route(path, route.method.actix_route().to(create_response));
            }
            StableInferenceOperation::DeleteResponse => {
                cfg.route(path, route.method.actix_route().to(delete_response));
            }
            StableInferenceOperation::Embedding => {
                cfg.route(path, route.method.actix_route().to(embeddings));
            }
            StableInferenceOperation::GetModel => {
                cfg.route(path, route.method.actix_route().to(get_model));
            }
            StableInferenceOperation::GetResponse => {
                cfg.route(path, route.method.actix_route().to(get_response));
            }
            StableInferenceOperation::ImageEdit => {
                cfg.route(path, route.method.actix_route().to(image_edits));
            }
            StableInferenceOperation::ImageGeneration => {
                cfg.route(path, route.method.actix_route().to(image_generations));
            }
            StableInferenceOperation::ImageVariation => {
                cfg.route(path, route.method.actix_route().to(image_variations));
            }
            StableInferenceOperation::ListModels => {
                cfg.route(path, route.method.actix_route().to(list_models));
            }
            StableInferenceOperation::ListResponseInputItems => {
                cfg.route(
                    path,
                    route.method.actix_route().to(list_response_input_items),
                );
            }
            StableInferenceOperation::Moderation => {
                cfg.route(path, route.method.actix_route().to(create_moderation));
            }
            StableInferenceOperation::Rerank => {
                cfg.route(path, route.method.actix_route().to(rerank));
            }
        }
    }
}
