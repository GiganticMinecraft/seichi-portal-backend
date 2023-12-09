use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use chrono::Utc;
use domain::{
    form::models::{
        Comment, CommentSchema, Form, FormId, FormQuestionUpdateSchema, FormUpdateTargets,
        OffsetAndLimit, PostedAnswers, PostedAnswersSchema,
    },
    repository::Repositories,
    user::models::User,
};
use errors::{infra::InfraError, Error};
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::form::FormUseCase;

pub async fn create_form_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Json(form): Json<Form>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    match form_use_case
        .create_form(form.title, form.description, user)
        .await
    {
        Ok(id) => (
            StatusCode::CREATED,
            [(
                header::LOCATION,
                HeaderValue::from_str(id.to_string().as_str()).unwrap(),
            )],
            Json(json!({ "id": id })),
        )
            .into_response(),
        Err(err) => {
            tracing::error!("{}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "reason": "unknown error" })),
            )
                .into_response()
        }
    }
}

pub async fn form_list_handler(
    State(repository): State<RealInfrastructureRepository>,
    Query(offset_and_limit): Query<OffsetAndLimit>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    match form_use_case.form_list(offset_and_limit).await {
        Ok(forms) => (StatusCode::OK, Json(forms)).into_response(),
        Err(err) => {
            tracing::error!("{}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "reason": "unknown error" })),
            )
                .into_response()
        }
    }
}

pub async fn get_form_handler(
    State(repository): State<RealInfrastructureRepository>,
    Path(form_id): Path<FormId>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    match form_use_case.get_form(form_id).await {
        Ok(form) => (StatusCode::OK, Json(form)).into_response(),
        Err(err) => {
            tracing::error!("{}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "reason": "unknown error" })),
            )
                .into_response()
        }
    }
}

pub async fn delete_form_handler(
    State(repository): State<RealInfrastructureRepository>,
    Path(form_id): Path<FormId>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    match form_use_case.delete_form(form_id).await {
        Ok(form_id) => (StatusCode::OK, Json(json!({ "id": form_id }))).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn update_form_handler(
    State(repository): State<RealInfrastructureRepository>,
    Path(form_id): Path<FormId>,
    Query(targets): Query<FormUpdateTargets>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    match form_use_case.update_form(form_id, targets).await {
        Ok(form) => (StatusCode::OK, Json(form)).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn get_questions_handler(
    State(repository): State<RealInfrastructureRepository>,
    Path(form_id): Path<FormId>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    match form_use_case.get_questions(form_id).await {
        Ok(questions) => (StatusCode::OK, Json(questions)).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn get_all_answers(
    State(repository): State<RealInfrastructureRepository>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    match form_use_case.get_all_answers().await {
        Ok(answers) => (StatusCode::OK, Json(answers)).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn post_answer_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Json(schema): Json<PostedAnswersSchema>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    let answers = PostedAnswers {
        uuid: user.id,
        timestamp: Utc::now(),
        form_id: schema.form_id,
        title: schema.title,
        answers: schema.answers,
    };

    match form_use_case.post_answers(answers).await {
        Ok(_) => (StatusCode::OK).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn create_question_handler(
    State(repository): State<RealInfrastructureRepository>,
    Json(questions): Json<FormQuestionUpdateSchema>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    match form_use_case.create_questions(questions).await {
        Ok(_) => (StatusCode::CREATED).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn post_form_comment(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Json(comment_schema): Json<CommentSchema>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    let comment = Comment {
        answer_id: comment_schema.answer_id,
        content: comment_schema.content,
        timestamp: chrono::Utc::now(),
        commented_by: user,
    };

    match form_use_case.post_comment(comment).await {
        Ok(_) => (StatusCode::OK).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub fn handle_error(err: Error) -> impl IntoResponse {
    match err {
        Error::Infra {
            source: InfraError::FormNotFound { .. },
        } => (
            StatusCode::NOT_FOUND,
            Json(json!({ "reason": "FORM NOT FOUND" })),
        )
            .into_response(),
        Error::Infra {
            source: InfraError::Forbidden,
        } => StatusCode::FORBIDDEN.into_response(),
        _ => {
            tracing::error!("{}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "reason": "unknown error" })),
            )
                .into_response()
        }
    }
}
