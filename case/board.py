"""Board facts, measured from Waveshare's STEP model (docs/ESP32-S3-Touch-AMOLED-1.75-3D.zip).

All values are in the case frame described in params.py. `check.py` loads the STEP itself
and tests the printed parts against it, so these numbers only have to be right enough to
design with.
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
UNDER_BOARD_Z = (
    -8.20
)  # lowest part over the middle (x +-11, y -17 to 13.5), where a speaker or cell lies
PLUG_ENTRIES = (
    (4.24, 15.92, 8.74, 18.72),
    (-9.06, 15.92, -4.56, 18.72),
)  # sockets' open ends: x0, y0, x1, y1

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
    """The full board, in the case frame. The STEP is unpacked from docs/ on first use."""
    step = CACHE / STEP_NAME
    if not step.exists():
        CACHE.mkdir(exist_ok=True)
        with zipfile.ZipFile(STEP_ZIP) as z:
            step.write_bytes(z.read(STEP_NAME))
    return MODEL_TO_CASE * import_step(step)
