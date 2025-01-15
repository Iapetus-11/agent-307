# Agent 307
*A small tool to record video in segments for security purposes*

<img width="1512" alt="image" src="https://github.com/user-attachments/assets/2ad68192-cf50-4ca2-8bfa-da86bfcc8011" />


## Checklist
- [x] Show video previews in-app
- [x] Record/save video in segments
- [x] Auto Delete footage older than certain time
- [ ] Video segment length config
- [ ] In-app config management

## MacOS Build Instructions
```
export DYLD_FALLBACK_LIBRARY_PATH="/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/lib"
export LDFLAGS=-L/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/lib
export LD_LIBRARY_PATH=${LD_LIBRARY_PATH}:/usr/local/lib

cargo build
```
