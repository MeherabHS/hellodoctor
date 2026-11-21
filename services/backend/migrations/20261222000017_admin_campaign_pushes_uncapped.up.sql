-- Admin-published notifications (ADMIN_CAMPAIGN) were capped at 3 pushes per
-- user per day (20261117000001). From the 4th of the day the push was
-- suppressed permanently while the in-app row still appeared, so admin
-- announcements showed in the inbox but never pinged the device (product
-- owner, 2026-11-21). Admin campaigns are deliberate and few, so the cap is
-- lifted to the registry maximum. The user's own engagement opt-out and quiet
-- hours (off unless the user turns them on) still apply.
UPDATE notification_type_registry SET max_per_day = 1000 WHERE notification_type = 'ADMIN_CAMPAIGN';
