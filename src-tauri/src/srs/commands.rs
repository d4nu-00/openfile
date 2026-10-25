//! The Tauri commands of the spaced repetition decks.
use crate::{
    clipboard::RowRange,
    commands::{AppError, blocking},
    registry::Registry,
    srs::{
        deck::{DeckOptions, Grade},
        store::{
            AnswerResponse, CreateResponse, DeckDetail, DeckListing, QueueResponse, SrsStore,
            UpdateResponse,
        },
    },
};
use std::{path::PathBuf, sync::Arc};

type Store<'a> = tauri::State<'a, Arc<SrsStore>>;

#[tauri::command]
pub async fn srs_decks(
    store: Store<'_>,
    now: i64,
    utc_offset: i32,
) -> Result<DeckListing, AppError> {
    let s = store.inner().clone();
    blocking(move || s.decks(now, utc_offset)).await
}

#[tauri::command]
pub async fn srs_create_from_pgn(
    store: Store<'_>,
    path: PathBuf,
    name: String,
    options: DeckOptions,
    now: i64,
    utc_offset: i32,
) -> Result<CreateResponse, AppError> {
    let s = store.inner().clone();
    blocking(move || s.create_from_pgn(path, name, options, now, utc_offset)).await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn srs_create_from_rows(
    store: Store<'_>,
    registry: tauri::State<'_, Arc<Registry>>,
    database_id: String,
    ranges: Vec<RowRange>,
    name: String,
    options: DeckOptions,
    now: i64,
    utc_offset: i32,
) -> Result<CreateResponse, AppError> {
    let (s, r) = (store.inner().clone(), registry.inner().clone());
    blocking(move || s.create_from_rows(&r, database_id, ranges, name, options, now, utc_offset))
        .await
}

#[tauri::command]
pub async fn srs_update_from_pgn(
    store: Store<'_>,
    deck_id: String,
    path: PathBuf,
    now: i64,
    utc_offset: i32,
) -> Result<UpdateResponse, AppError> {
    let s = store.inner().clone();
    blocking(move || s.update_from_pgn(&deck_id, path, now, utc_offset)).await
}

#[tauri::command]
pub async fn srs_update_from_rows(
    store: Store<'_>,
    registry: tauri::State<'_, Arc<Registry>>,
    deck_id: String,
    database_id: String,
    ranges: Vec<RowRange>,
    now: i64,
    utc_offset: i32,
) -> Result<UpdateResponse, AppError> {
    let (s, r) = (store.inner().clone(), registry.inner().clone());
    blocking(move || s.update_from_rows(&r, &deck_id, database_id, ranges, now, utc_offset)).await
}

#[tauri::command]
pub async fn srs_deck(store: Store<'_>, deck_id: String) -> Result<DeckDetail, AppError> {
    let s = store.inner().clone();
    blocking(move || s.deck(&deck_id)).await
}

#[tauri::command]
pub async fn srs_set_options(
    store: Store<'_>,
    deck_id: String,
    options: DeckOptions,
) -> Result<(), AppError> {
    let s = store.inner().clone();
    blocking(move || s.set_options(&deck_id, options)).await
}

#[tauri::command]
pub async fn srs_rename(store: Store<'_>, deck_id: String, name: String) -> Result<(), AppError> {
    let s = store.inner().clone();
    blocking(move || s.rename(&deck_id, name)).await
}

#[tauri::command]
pub async fn srs_delete(store: Store<'_>, deck_id: String) -> Result<(), AppError> {
    let s = store.inner().clone();
    blocking(move || s.delete(&deck_id)).await
}

#[tauri::command]
pub async fn srs_queue(
    store: Store<'_>,
    deck_id: String,
    now: i64,
    utc_offset: i32,
) -> Result<QueueResponse, AppError> {
    let s = store.inner().clone();
    blocking(move || s.queue(&deck_id, now, utc_offset)).await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn srs_answer(
    store: Store<'_>,
    deck_id: String,
    card_id: String,
    grade: Grade,
    mistakes: u32,
    hints: u32,
    millis: u64,
    now: i64,
    utc_offset: i32,
) -> Result<AnswerResponse, AppError> {
    let s = store.inner().clone();
    blocking(move || {
        s.answer(
            &deck_id, &card_id, grade, mistakes, hints, millis, now, utc_offset,
        )
    })
    .await
}

#[tauri::command]
pub async fn srs_suspend(
    store: Store<'_>,
    deck_id: String,
    card_id: String,
    suspended: bool,
) -> Result<(), AppError> {
    let s = store.inner().clone();
    blocking(move || s.suspend(&deck_id, &card_id, suspended)).await
}
