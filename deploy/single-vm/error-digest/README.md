# Nightly error digest

One email every night at **03:00 Africa/Nairobi** with everything that went
wrong on the VM in the last 24 hours. On a quiet night it still sends an
"all clear", so silence always means the bot itself has a problem.

| Area | What is checked |
|---|---|
| Backend and app errors | `system_error_logs`: server faults and crash reports sent by the apps |
| Email delivery | Failed or expired emails, and emails stuck in the queue over 30 minutes |
| Push and in-app notifications | Failed notification deliveries |
| Background workers | Erasure, booking-expiry and document-removal jobs that keep failing |
| Database | Reachable, connection use, queries running over 5 minutes |
| Containers | Stopped, unhealthy or restarting services; ERROR/panic lines in their logs |
| VM | Disk over 80%, low memory, high load, a reboot in the last 24 hours |

It is read-only: database queries run in a read-only transaction through
`docker exec`, so no database password is stored. Only counts, components
and error text are reported, never user ids or recipients, and email
addresses, phone numbers, tokens and long numbers are masked before
anything is sent.

## Install on the VM (Ubuntu 24.04)

```sh
sudo useradd --system --no-create-home --groups docker hellodoctor-digest
sudo mkdir -p /opt/hellodoctor/error-digest /etc/hellodoctor
sudo cp error_digest.py /opt/hellodoctor/error-digest/
sudo cp error-digest.env.example /etc/hellodoctor/error-digest.env
sudo nano /etc/hellodoctor/error-digest.env          # your address and SMTP account
printf '%s' 'the-smtp-app-password' | sudo tee /etc/hellodoctor/error-digest-smtp-password >/dev/null
sudo chown root:hellodoctor-digest /etc/hellodoctor/error-digest.env /etc/hellodoctor/error-digest-smtp-password
sudo chmod 640 /etc/hellodoctor/error-digest.env /etc/hellodoctor/error-digest-smtp-password
sudo cp error-digest.service error-digest.timer /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now error-digest.timer
```

Try it before waiting for 03:00:

```sh
sudo -u hellodoctor-digest env $(sudo cat /etc/hellodoctor/error-digest.env | xargs) \
  python3 /opt/hellodoctor/error-digest/error_digest.py --dry-run   # prints, sends nothing
sudo systemctl start error-digest.service                            # sends one now
systemctl list-timers error-digest.timer                              # next run
journalctl -u error-digest.service -n 20                              # last result
```

**Gmail as the sender:** turn on 2-Step Verification for the sending
account, create an App password (Google Account → Security → App
passwords), and put that in the password file. Any other SMTP provider works
the same way (port 465 uses SSL, other ports use STARTTLS).

## Tests

```sh
python3 -m unittest test_error_digest.py
```
