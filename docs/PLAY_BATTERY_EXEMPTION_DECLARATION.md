# Play Console: battery-optimisation exemption declaration

Both Android apps (the Care app `com.hellodoctor.doctor` and the Patient app
`com.hellodoctor.patient`) declare
`android.permission.REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` (product-owner
decision, 2026-11-20). They use it to show Android's own one-tap dialog, "Let
app always run in background?". The user must tap **Allow**; the app cannot
grant the exemption itself.

Google Play restricts this permission to apps whose core function would break
under battery optimisation. Each app's release needs the declaration below in
Play Console (App content → Sensitive permissions, or as prompted during
review).

## Text to submit (Care app)

> AfyaMkononi Care is used by licensed doctors and trained Peer Navigators to
> deliver scheduled telehealth consultations. Its core function depends on
> time-critical alerts delivered while the app is in the background: a
> reminder 15 minutes before each booked video or chat consultation, new
> patient messages during a consultation window that closes at the end of the
> booked slot, and new peer-support requests that must be accepted in time.
> Many devices in our market (Kenya: Tecno, Infinix, itel, Xiaomi, Oppo and
> Vivo) stop background apps aggressively, which delays or drops these alerts
> and causes missed consultations. The app shows the system dialog only when the
> device is restricting the app, does not ask again sooner than about once a
> day, and remains fully usable if the user declines. Notifications never contain health information.

## Text to submit (Patient app)

> AfyaMkononi lets young people book video and chat consultations with
> licensed doctors and set personal health reminders (for example, medication
> times). Its core function depends on time-critical background delivery: a
> reminder 15 minutes before each booked consultation, doctor messages during
> a chat consultation that closes at the end of the booked slot, and the
> user's own scheduled reminders. Many devices in our market (Kenya: Tecno,
> Infinix, itel, Xiaomi, Oppo and Vivo) stop background apps aggressively,
> which delays or drops these alerts. The app shows the system dialog only when the
> device is restricting the app, does not ask again sooner than about once a
> day, and remains fully usable if the user declines. Notifications never contain health information.

## If Google refuses

1. Remove the `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` line from that app's
   `android/app/src/main/AndroidManifest.xml`.
2. The battery dialog then stops appearing. The notification dialog is
   unaffected. As the product owner instructed, the app never redirects the
   user to a Settings page instead.

## What the apps do (product owner, 2026-11-20: two taps, nothing else)

After sign-in (and after any first-login onboarding),
`BatteryOptimization.ensureReady` shows Android's own dialogs back to back,
with no screens of the app's own:

1. "Allow AfyaMkononi to send you notifications?" (Android 13 and later,
   while Android still shows it).
2. "Let app always run in background?"

- **Only what is still off is asked.** A refusal is asked again on a later
  launch, at most once every 20 hours.
- **No redirects.** The user is never sent to a Settings page.
- **What stays outside the app's control.** Auto-start on Tecno, Xiaomi and
  similar phones has no Android API, so it remains the user's own setting.
