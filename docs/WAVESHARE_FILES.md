# Waveshare's board files

Waveshare publishes the board's schematic and 3D models. Download them into `docs/` under these
names:

| Save as | What it is | Where to get it |
|---|---|---|
| `ESP32-S3-Touch-AMOLED-1.75-3D.zip` | The board's 3D model (STEP). `case/` needs it for the fit test and renders. | [files.waveshare.com](https://files.waveshare.com/wiki/ESP32-S3-Touch-AMOLED-1.75/ESP32-S3-Touch-AMOLED-1.75-3D.zip) |
| `ESP32-S3-Touch-AMOLED-1.75-B-3D.zip` | Waveshare's second 3D model download (the "-B" file) | [files.waveshare.com](https://files.waveshare.com/wiki/ESP32-S3-Touch-AMOLED-1.75/ESP32-S3-Touch-AMOLED-1.75-B-3D.zip) |
| `ESP32-S3-Touch-AMOLED-1.75.pdf` | The schematic | [Waveshare's GitHub](https://github.com/waveshareteam/ESP32-S3-Touch-AMOLED-1.75/blob/main/Schematic/ESP32-S3-Touch-AMOLED-1.75-schematic.pdf) |

All three at once:

```sh
cd docs
curl -LO https://files.waveshare.com/wiki/ESP32-S3-Touch-AMOLED-1.75/ESP32-S3-Touch-AMOLED-1.75-3D.zip
curl -LO https://files.waveshare.com/wiki/ESP32-S3-Touch-AMOLED-1.75/ESP32-S3-Touch-AMOLED-1.75-B-3D.zip
curl -L -o ESP32-S3-Touch-AMOLED-1.75.pdf \
  https://raw.githubusercontent.com/waveshareteam/ESP32-S3-Touch-AMOLED-1.75/main/Schematic/ESP32-S3-Touch-AMOLED-1.75-schematic.pdf
```

More from Waveshare: the [wiki page](https://www.waveshare.com/wiki/ESP32-S3-Touch-AMOLED-1.75)
and the [GitHub repo](https://github.com/waveshareteam/ESP32-S3-Touch-AMOLED-1.75).
