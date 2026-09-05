//! Migration: create metadata-only request_ledger table.
//!
//! Retention is enforced on the write path using
//! `storage.request_ledger.retention_days`, throttled so expired rows are not
//! deleted on every request.
//! Indexes are bounded to time-range, request id, model, provider, and terminal
//! status filters used by the later admin query API.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RequestLedger::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RequestLedger::RequestId)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RequestLedger::StartedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RequestLedger::FinishedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(RequestLedger::Method).string().not_null())
                    .col(ColumnDef::new(RequestLedger::Endpoint).string().not_null())
                    .col(ColumnDef::new(RequestLedger::Model).string().null())
                    .col(ColumnDef::new(RequestLedger::Provider).string().null())
                    .col(ColumnDef::new(RequestLedger::Deployment).string().null())
                    .col(
                        ColumnDef::new(RequestLedger::StatusCode)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RequestLedger::TerminalStatus)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RequestLedger::LatencyMs)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RequestLedger::PromptTokens)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(RequestLedger::CompletionTokens)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(RequestLedger::TotalTokens)
                            .big_integer()
                            .null(),
                    )
                    .col(ColumnDef::new(RequestLedger::Cost).double().null())
                    .col(ColumnDef::new(RequestLedger::UserId).string().null())
                    .col(ColumnDef::new(RequestLedger::ApiKeyId).string().null())
                    .col(ColumnDef::new(RequestLedger::TeamId).string().null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_request_ledger_finished_at")
                    .table(RequestLedger::Table)
                    .col(RequestLedger::FinishedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_request_ledger_model_finished_at")
                    .table(RequestLedger::Table)
                    .col(RequestLedger::Model)
                    .col(RequestLedger::FinishedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_request_ledger_provider_finished_at")
                    .table(RequestLedger::Table)
                    .col(RequestLedger::Provider)
                    .col(RequestLedger::FinishedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_request_ledger_status_finished_at")
                    .table(RequestLedger::Table)
                    .col(RequestLedger::TerminalStatus)
                    .col(RequestLedger::FinishedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RequestLedger::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum RequestLedger {
    Table,
    RequestId,
    StartedAt,
    FinishedAt,
    Method,
    Endpoint,
    Model,
    Provider,
    Deployment,
    StatusCode,
    TerminalStatus,
    LatencyMs,
    PromptTokens,
    CompletionTokens,
    TotalTokens,
    Cost,
    UserId,
    ApiKeyId,
    TeamId,
}
