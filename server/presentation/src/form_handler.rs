use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use domain::{
    form::models::{
        AnswerId, Comment, CommentId, CommentSchema, Form, FormId, FormQuestionUpdateSchema,
        FormUpdateTargets, Label, LabelId, LabelSchema, OffsetAndLimit, PostedAnswersSchema,
        PostedAnswersUpdateSchema, ReplaceAnswerLabelSchema,
    },
    repository::Repositories,
    user::models::User,
};
use errors::{infra::InfraError, usecase::UseCaseError, Error};
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
    Json(targets): Json<FormUpdateTargets>,
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

pub async fn get_answer_handler(
    State(repository): State<RealInfrastructureRepository>,
    Path(answer_id): Path<AnswerId>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    match form_use_case.get_answers(answer_id).await {
        Ok(answer) => (StatusCode::OK, Json(answer)).into_response(),
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

    match form_use_case.post_answers(&user, &schema).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn update_answer_handler(
    State(repository): State<RealInfrastructureRepository>,
    Path(answer_id): Path<AnswerId>,
    Json(schema): Json<PostedAnswersUpdateSchema>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    match form_use_case.update_answer_meta(answer_id, &schema).await {
        Ok(_) => StatusCode::OK.into_response(),
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

    match form_use_case.create_questions(&questions).await {
        Ok(_) => (StatusCode::CREATED, Json(json!({"id": questions.form_id }))).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn put_question_handler(
    State(repository): State<RealInfrastructureRepository>,
    Json(questions): Json<FormQuestionUpdateSchema>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    match form_use_case.put_questions(&questions).await {
        Ok(_) => (StatusCode::OK, Json(json!({"id": questions.form_id }))).into_response(),
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
        // NOTE: コメントはデータベースで insert した後に id が振られるのでデフォルト値を入れておく
        comment_id: Default::default(),
        content: comment_schema.content,
        timestamp: chrono::Utc::now(),
        commented_by: user,
    };

    match form_use_case
        .post_comment(comment, comment_schema.answer_id)
        .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn delete_form_comment_handler(
    State(repository): State<RealInfrastructureRepository>,
    Path(comment_id): Path<CommentId>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    match form_use_case.delete_comment(comment_id).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn create_label_for_answers(
    State(repository): State<RealInfrastructureRepository>,
    Json(label): Json<LabelSchema>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    match form_use_case.create_label_for_answers(&label).await {
        Ok(_) => StatusCode::CREATED.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn get_labels_for_answers(
    State(repository): State<RealInfrastructureRepository>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    match form_use_case.get_labels_for_answers().await {
        Ok(labels) => (StatusCode::OK, Json(labels)).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn delete_label_for_answers(
    State(repository): State<RealInfrastructureRepository>,
    Path(label_id): Path<LabelId>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    match form_use_case.delete_label_for_answers(label_id).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn edit_label_for_answers(
    State(repository): State<RealInfrastructureRepository>,
    Path(label_id): Path<LabelId>,
    Json(label): Json<LabelSchema>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    match form_use_case
        .edit_label_for_answers(&Label {
            id: label_id,
            name: label.name,
        })
        .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn replace_answer_labels(
    State(repository): State<RealInfrastructureRepository>,
    Path(answer_id): Path<AnswerId>,
    Json(label_ids): Json<ReplaceAnswerLabelSchema>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    match form_use_case
        .replace_answer_labels(answer_id, label_ids.labels)
        .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn create_label_for_forms(
    State(repository): State<RealInfrastructureRepository>,
    Json(label): Json<LabelSchema>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    match form_use_case.create_label_for_forms(&label).await {
        Ok(_) => StatusCode::CREATED.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn get_labels_for_forms(
    State(repository): State<RealInfrastructureRepository>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    match form_use_case.get_labels_for_forms().await {
        Ok(labels) => (StatusCode::OK, Json(labels)).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub fn handle_error(err: Error) -> impl IntoResponse {
    match err {
        Error::Infra {
            source: InfraError::FormNotFound { .. },
        } => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "errorCode": "FORM_NOT_FOUND",
                "reason": "FORM NOT FOUND"
            })),
        )
            .into_response(),
        Error::UseCase {
            source: UseCaseError::AnswerNotFound,
        } => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "errorCode": "ANSWER_NOT_FOUND",
                "reason": "Answer not found"
            })),
        )
            .into_response(),
        Error::UseCase {
            source: UseCaseError::OutOfPeriod,
        } => (
            StatusCode::FORBIDDEN,
            Json(json!({
                "errorCode": "OUT_OF_PERIOD",
                "reason": "Posted form is out of period."
            })),
        )
            .into_response(),
        Error::UseCase {
            source: UseCaseError::DoNotHavePermissionToPostFormComment,
        } => (
            StatusCode::FORBIDDEN,
            Json(json!({
                "errorCode": "DO_NOT_HAVE_PERMISSION_TO_POST_FORM_COMMENT",
                "reason": "Do not have permission to post form comment."
            })),
        )
            .into_response(),
        _ => {
            tracing::error!("{}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "errorCode": "INTERNAL_SERVER_ERROR",
                    "reason": "unknown error"
                })),
            )
                .into_response()
        }
    }
}
