DROP TRIGGER IF EXISTS campaign_update_guard ON campaigns;
DROP FUNCTION IF EXISTS guard_campaign_update();
DROP TABLE IF EXISTS campaigns;
DROP TYPE IF EXISTS campaign_status;
DROP TYPE IF EXISTS campaign_audience;
