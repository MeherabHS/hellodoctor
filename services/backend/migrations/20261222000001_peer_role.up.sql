-- Peer Navigator role (product-owner instruction 2026-11-14). Its own
-- migration: PostgreSQL cannot use a newly added enum value until commit.
ALTER TYPE user_role_enum ADD VALUE IF NOT EXISTS 'PEER';
