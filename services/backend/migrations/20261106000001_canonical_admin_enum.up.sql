-- Separate transaction: PostgreSQL cannot use a newly added enum value until commit.
ALTER TYPE user_role_enum ADD VALUE IF NOT EXISTS 'ADMIN';
