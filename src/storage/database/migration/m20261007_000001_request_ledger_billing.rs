//! Extend the existing ledger without fabricating accounting facts for old rows.
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for column in [RequestLedger::Billing, RequestLedger::Reconciliation] {
            manager
                .alter_table(
                    Table::alter()
                        .table(RequestLedger::Table)
                        .add_column(ColumnDef::new(column).json().null())
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for column in [RequestLedger::Reconciliation, RequestLedger::Billing] {
            manager
                .alter_table(
                    Table::alter()
                        .table(RequestLedger::Table)
                        .drop_column(column)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

#[derive(DeriveIden)]
enum RequestLedger {
    Table,
    Billing,
    Reconciliation,
}
