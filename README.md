
# Live Collab

A DAW plugin for sharing audio packets from a mixer channel to someone elses mixer channel through WebRTC.
<br/>This is a free alternative to the Waves Stream plugin.


## Installation

Install Rust & Cargo: https://doc.rust-lang.org/cargo/getting-started/installation.html

Build Command:
```bash
cargo xtask
```

The compiled VST3 and CLAP files will be in target/bundled. Put the VST3 in your DAW's VST3 folder and scan for new plugins in the DAW.


## Usage

- Sender will add the live-collab-sender plugin to their mixer channel<br/>
![Step1](https://github.com/user-attachments/assets/e4f60f9b-152d-4bc6-9581-3e256e520148)
- Receiver will add the live-collab-receiver plugin to their mixer channel<br/>
![Step2](https://github.com/user-attachments/assets/c78c2fce-9ab2-4156-90b3-afc90d4c552a)

- Sender will click "Create Session" and then click "Copy Session Token"<br/>
![Step3](https://github.com/user-attachments/assets/8f1e850c-aeca-45d7-8320-047b96d5c529) ![Screenshot_9](https://github.com/user-attachments/assets/8a792dda-825f-4f25-a7c4-a8bda84318c0)

- The session token is now copied to the sender's clipboard, and the sender must send it to the receiver
- Once the receiver has the session token, they will paste it into the Peer offer section and click "Connect"
  - If it fails to connect, then one of the machines are not able to use WebRTC through STUN only and a TURN server will be needed
![Step5](https://github.com/user-attachments/assets/e44c1582-804e-4992-af76-6f79ef7ad4d1)

- On a successful connection, the receiver must click "Copy Session Token" and send their session token back to the sender
![Step6](https://github.com/user-attachments/assets/e3dcb27a-a2c0-4e14-b365-b48612d844a7)

- Once the sender receives the session token, they must put it in the Peer answer section and click "Set Answer" to finalize the connection
![Step7](https://github.com/user-attachments/assets/c491f261-b3c8-40e2-846d-96777720f21c)

- Connection state should now be "connected" and audio should be transmitting
  - In the image below, you can see there is no input selected for the channel with the receiver. It is playing audio because it's receiving the audio packets from the sender.
![Step8](https://github.com/user-attachments/assets/bbaaec69-7a51-455b-b685-ea84b632f1d0)