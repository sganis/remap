:: Build gtk tools
:: Run from admin powershell terminal

:: chocolatey
Set-ExecutionPolicy Bypass -Scope Process -Force; iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))

:: msys2
choco install msys2 -y

:: visual studio 2022 tools
choco install visualstudio2022-workload-vctools -y

:: from normal user terminal
mkdir C:\gtk-build\github
cd C:\gtk-build\github
git clone https://github.com/sganis/gvsbuild.git
cd C:\gtk-build\github\gvsbuild
pip install .


gvsbuild build openssl
gvsbuild build cargo
gvsbuild build gtk4



