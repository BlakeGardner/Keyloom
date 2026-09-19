# Keyloom

**Your keyboard, the way you want it.**

Keyloom is a visual keyboard customization app for Linux. Click a key,
choose what it should do, and it takes effect right away. Simple things
stay simple, with real power when you want it.

**[Install Keyloom →](#install-keyloom)**

## Remap keys visually

Pick a key and give it a new job. Here F7, F8, and F9 become media
controls like Previous, Play/Pause, and Next.


https://github.com/user-attachments/assets/3785f148-b780-4f1a-956a-c0d8ba299605

## Remap keys in one application

A key can behave differently in one application and stay normal
everywhere else. Here Caps Lock becomes Escape in Visual Studio Code.

https://github.com/user-attachments/assets/39a8468d-6dc6-4d0e-a82e-4ccac93a25f0

## Test your keyboard

See keys light up as you press them, and what each one sends. Keyloom
works out your keyboard's size and layout for you.

https://github.com/user-attachments/assets/2430f87a-9767-4bc9-8b00-454ced1da319

## More in Keyloom

- **Tap and hold**: Escape when tapped, Control when held.
- **Layers**: hold Caps Lock and H/J/K/L become arrow keys, while every
  other key keeps working normally.
- **Shortcuts**: turn one chord into another, so Super+C sends the
  terminal's Ctrl+Shift+C.
- **Profiles**: keep separate setups for work, gaming, or Mac-style
  controls and switch as your day changes.
- **One keyboard at a time**: customize your laptop and external keyboards
  independently.
- **Unusual keys too**: media, brightness, and Apple's Mission Control and
  Launchpad keys.
- **Changes apply themselves**: no Apply button, and one click pauses
  remapping when you want your original keys back.

## Install Keyloom

Add Keyloom's repository for your distribution, and Keyloom updates with
the rest of your system. Pick yours below for the commands.

### Debian

<details>
<summary><b>Debian 13</b></summary>

```sh
echo 'deb http://download.opensuse.org/repositories/home:/BlakeGardner:/Keyloom/Debian_13/ /' | sudo tee /etc/apt/sources.list.d/home:BlakeGardner:Keyloom.list
curl -fsSL https://download.opensuse.org/repositories/home:BlakeGardner:Keyloom/Debian_13/Release.key | gpg --dearmor | sudo tee /etc/apt/trusted.gpg.d/home_BlakeGardner_Keyloom.gpg > /dev/null
sudo apt update
sudo apt install keyloom
```

</details>

<details>
<summary><b>Debian testing</b></summary>

```sh
echo 'deb http://download.opensuse.org/repositories/home:/BlakeGardner:/Keyloom/Debian_Testing/ /' | sudo tee /etc/apt/sources.list.d/home:BlakeGardner:Keyloom.list
curl -fsSL https://download.opensuse.org/repositories/home:BlakeGardner:Keyloom/Debian_Testing/Release.key | gpg --dearmor | sudo tee /etc/apt/trusted.gpg.d/home_BlakeGardner_Keyloom.gpg > /dev/null
sudo apt update
sudo apt install keyloom
```

</details>

### Ubuntu / Pop!_OS

Pop!_OS builds on Ubuntu, so it uses the repository for the Ubuntu
release underneath it: Pop!_OS 24.04 takes the Ubuntu 24.04 commands.

<details>
<summary><b>Ubuntu 24.04 or Pop!_OS 24.04</b></summary>

```sh
echo 'deb http://download.opensuse.org/repositories/home:/BlakeGardner:/Keyloom/xUbuntu_24.04/ /' | sudo tee /etc/apt/sources.list.d/home:BlakeGardner:Keyloom.list
curl -fsSL https://download.opensuse.org/repositories/home:BlakeGardner:Keyloom/xUbuntu_24.04/Release.key | gpg --dearmor | sudo tee /etc/apt/trusted.gpg.d/home_BlakeGardner_Keyloom.gpg > /dev/null
sudo apt update
sudo apt install keyloom
```

</details>

<details>
<summary><b>Ubuntu 25.10</b></summary>

```sh
echo 'deb http://download.opensuse.org/repositories/home:/BlakeGardner:/Keyloom/xUbuntu_25.10/ /' | sudo tee /etc/apt/sources.list.d/home:BlakeGardner:Keyloom.list
curl -fsSL https://download.opensuse.org/repositories/home:BlakeGardner:Keyloom/xUbuntu_25.10/Release.key | gpg --dearmor | sudo tee /etc/apt/trusted.gpg.d/home_BlakeGardner_Keyloom.gpg > /dev/null
sudo apt update
sudo apt install keyloom
```

</details>

<details>
<summary><b>Ubuntu 26.04</b></summary>

```sh
echo 'deb http://download.opensuse.org/repositories/home:/BlakeGardner:/Keyloom/xUbuntu_26.04/ /' | sudo tee /etc/apt/sources.list.d/home:BlakeGardner:Keyloom.list
curl -fsSL https://download.opensuse.org/repositories/home:BlakeGardner:Keyloom/xUbuntu_26.04/Release.key | gpg --dearmor | sudo tee /etc/apt/trusted.gpg.d/home_BlakeGardner_Keyloom.gpg > /dev/null
sudo apt update
sudo apt install keyloom
```

</details>

### Fedora

<details>
<summary><b>Fedora 43</b></summary>

```sh
sudo dnf config-manager addrepo --from-repofile=https://download.opensuse.org/repositories/home:BlakeGardner:Keyloom/Fedora_43/home:BlakeGardner:Keyloom.repo
sudo dnf install keyloom
```

</details>

<details>
<summary><b>Fedora 44</b></summary>

```sh
sudo dnf config-manager addrepo --from-repofile=https://download.opensuse.org/repositories/home:BlakeGardner:Keyloom/Fedora_44/home:BlakeGardner:Keyloom.repo
sudo dnf install keyloom
```

</details>

<details>
<summary><b>Fedora Rawhide</b></summary>

```sh
sudo dnf config-manager addrepo --from-repofile=https://download.opensuse.org/repositories/home:BlakeGardner:Keyloom/Fedora_Rawhide/home:BlakeGardner:Keyloom.repo
sudo dnf install keyloom
```

</details>

Prefer a one-off download? The
[releases page](https://github.com/BlakeGardner/Keyloom/releases) carries
a `.deb` and an `.rpm` for each of these, for 64-bit Intel/AMD and ARM,
along with the release notes.

Keyloom is in early development. It does its remapping through
[xremap](https://github.com/xremap/xremap): a guided first-run setup
downloads it if it isn't installed, then takes care of the background
service and input permissions.

## What's next

See the [upcoming features roadmap](docs/Upcoming_Features.md) for plans
beyond the first release, including more keyboard layouts, a translated
interface, and a log viewer.

## License

Keyloom is free and open source, licensed under [GPL-3.0-only](LICENSE).
