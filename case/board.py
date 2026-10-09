"""Board facts, measured from Waveshare's STEP model (docs/ESP32-S3-Touch-AMOLED-1.75-3D.zip).

Values are in mm, in the case frame described in params.py. check.py tests the parts against
the STEP itself, so these only have to be close enough to design with.
"""

import zipfile
from pathlib import Path

from build123d import Compound, Pos, Rot, import_step

HERE = Path(__file__).parent
STEP_ZIP = HERE.parent / "docs" / "ESP32-S3-Touch-AMOLED-1.75-3D.zip"
STEP_NAME = "ESP32-S3-Touch-AMOLED-1_75.stp"
CACHE = HERE / ".cache"

# The STEP has +Y out of the glass and the glass front at Y = 2.77. Rotating +90 deg about X
# maps model Y to case Z and model Z to case -Y, which puts USB-C at 9 o'clock.
MODEL_TO_CASE = Pos(0, 0, -2.77) * Rot(90, 0, 0)

GLASS_R = 24.48  # cover glass, the widest part (D48.96)
GLASS_DEPTH = 2.77  # glass and panel are full width from Z = 0 down to here
BODY_R = 23.01  # behind the glass everything stays inside this radius, except USB-C
PCB_BACK_Z = -6.90
STANDOFF_END_Z = -10.40  # M2 SMT standoffs, 3.5 mm tall, threaded through
STANDOFF_D = 3.5
STANDOFFS = ((13.75, -14.70), (-13.75, -14.70), (0.0, 20.50))
DEEPEST_Z = -12.70  # 8-pin female header along the 6 o'clock edge
SOCKETS_Z = -11.70  # BAT and SPK MX1.25 sockets along the 12 o'clock edge
# Under the board nothing hangs below UNDER_BOARD_Z except these, as (x0, y0, x1, y1, lowest z).
# The standoffs aren't listed; the posts deal with them.
UNDER_BOARD_Z = -8.20
LOW_PARTS = (
    (-10.43, -19.88, 10.42, -17.48, -12.70),  # 8-pin header
    (2.69, 15.92, 10.29, 19.62, -11.70),  # BAT and SPK sockets
    (-10.61, 15.92, -3.01, 19.62, -11.70),
    (-23.79, -4.81, -16.26, 4.77, -10.15),  # USB-C
    (14.73, -13.80, 19.48, -8.82, -8.70),  # side keys
    (14.73, 8.82, 19.48, 13.80, -8.70),
)
PLUG_ENTRIES = (
    (4.24, 15.92, 8.74, 18.72),
    (-9.06, 15.92, -4.56, 18.72),
)  # sockets' open ends: x0, y0, x1, y1

# GPS (-G board): the LC76G is on the front of the PCB, under the display, out of the case's way.
# Its antenna plugs into the U.FL socket beside it (the other U.FL is Wi-Fi). The plug and its
# cable need a margin round the socket kept clear down to GPS_PLUG_Z.
GPS_SOCKET = (-15.07, -9.10, -12.07, -6.00)
GPS_PLUG_Z = PCB_BACK_Z - 2.7

# USB-C socket: angle (deg), radius of its face, centre height, and width x height.
USB_ANGLE = 180.0
USB_FACE_R = 23.79
USB_Z = -8.57
USB_SIZE = (8.94, 3.16)

# Side keys: the actuators (2.0 wide, 1.0 tall) point straight out from the centre.
PWR_ANGLE = 33.65  # 2 o'clock
BOOT_ANGLE = -33.87  # 4 o'clock, the lower one
KEY_TIP_R = 22.84
KEY_Z = -7.80

SD_ANGLE = 0.0  # microSD push-push socket between the keys, card inserted towards -X
SD_Z = -5.10  # card plane, just above the PCB front face
SD_OFFSET = 0.25  # card centre, along the rim


def load() -> Compound:
    """The whole board in the case frame. The STEP is unzipped into .cache/ on first use."""
    step = CACHE / STEP_NAME
    if not step.exists() and not STEP_ZIP.exists():
        raise SystemExit(f"Missing {STEP_ZIP}: download it as docs/WAVESHARE_FILES.md says.")
    if not step.exists():
        CACHE.mkdir(exist_ok=True)
        with zipfile.ZipFile(STEP_ZIP) as z:
            step.write_bytes(z.read(STEP_NAME))
    return MODEL_TO_CASE * import_step(step)
