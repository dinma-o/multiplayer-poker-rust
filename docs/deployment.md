# Deployment & Server Environment Notes (Phase 2)

This document summarizes constraints and considerations for deploying and demonstrating the PokerDUD server-client system based on instructor guidance.

---

## 🧠 Key Takeaway

We are **not deploying to a production/public server**.

We will run our system on:
- Local machines
- Lab machines
- Private LAN (hotspot / router)

So the system should be designed for **local network communication**, not internet-scale deployment.

---

## 🌐 Networking Environment

### Allowed Setup
- Local LAN (e.g., hotspot, lab router)
- Machines receive `192.168.X.Y` IP addresses
- All devices are on the same network

### Important Notes
- No need for SSH
- Safe to open listening ports (behind firewall)
- No need to deal with UofA public networking

### Implications for Design
- Use **TCP sockets**
- Bind server to:
  ```
  0.0.0.0:<port>
  ```
- Clients connect using:
  ```
  <server-local-ip>:<port>
  ```

---

## 🔒 HTTP vs TCP

- If using a browser:
  - HTTPS may be required
- If using CLI or custom client:
  - Plain TCP is fine

### Recommendation
Use:
- **Raw TCP + JSON protocol**

---

## 🖥️ Lab Machine Constraints

### Allowed
- Internet access
- Install Rust crates
- Run user-level programs

### Not Allowed
- No root access
- Limited background/system services

### Implications
- Avoid system dependencies
- Prefer self-contained Rust binaries
- Use JSON for storage

---

## 💾 Persistence Strategy

- Use local file (e.g., `accounts.json`)
- Load on startup
- Save after updates

Avoid external databases.

---

## 🚀 Deployment Workflow

1. Build:
   ```
   cargo build --release
   ```

2. Transfer binary:
   - USB / Git / SCP

3. Run server:
   ```
   ./poker_server
   ```

4. Connect client:
   ```
   ./poker_client --host 192.168.X.Y --port 8080
   ```

---

## 📶 Demo Setup

### Recommended
- Phone hotspot (easiest)

### Alternative
- Lab router

Avoid public WiFi.

---

## 🧪 Testing Strategy

- Test across multiple machines
- Use LAN IP (not localhost)
- Verify multiple clients

---

## ⚠️ Common Pitfalls

- Using 127.0.0.1 instead of 0.0.0.0
- Hardcoding localhost
- Not testing on LAN
- Using unsupported dependencies

---

## ✅ Final Checklist

- Server runs on lab machine
- Clients connect via LAN IP
- Multiple clients work
- Persistence works
- No root dependencies
- Tested before demo

---

## 🧠 Design Implications

- TCP-based architecture
- JSON messaging
- Centralized server
- Keep it simple
