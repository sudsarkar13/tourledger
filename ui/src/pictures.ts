// Turns a chosen or photographed picture into a JPEG small enough to store and
// sync, while a long receipt stays readable.

export interface Picture {
  /** The JPEG, base64. */
  data: string;
  url: string;
}

const SHORT_SIDE = 1200;
const LONG_SIDE = 4000;
/** Rust refuses more than 3 MB; this is that, as base64, with room to spare. */
const MAX_BASE64 = 3_500_000;

export const jpegUrl = (data: string) => `data:image/jpeg;base64,${data}`;

/** How much to shrink a picture of this size. Never enlarges. */
export function fitScale(width: number, height: number): number {
  return Math.min(1, SHORT_SIDE / Math.min(width, height), LONG_SIDE / Math.max(width, height));
}

export async function toJpeg(file: Blob): Promise<Picture> {
  // Phone photos are often stored sideways with a note saying so.
  const bitmap = await createImageBitmap(file, { imageOrientation: "from-image" }).catch(() => {
    throw new Error("That file is not a picture this app can read. Use a photo or screenshot.");
  });
  let scale = fitScale(bitmap.width, bitmap.height);
  const canvas = document.createElement("canvas");
  const context = canvas.getContext("2d");
  if (!context) throw new Error("The picture could not be processed.");
  for (let quality = 0.82; ; quality -= 0.12, scale *= 0.8) {
    canvas.width = Math.max(1, Math.round(bitmap.width * scale));
    canvas.height = Math.max(1, Math.round(bitmap.height * scale));
    // Transparent screenshots would otherwise turn black.
    context.fillStyle = "#fff";
    context.fillRect(0, 0, canvas.width, canvas.height);
    context.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
    const url = canvas.toDataURL("image/jpeg", quality);
    const data = url.slice(url.indexOf(",") + 1);
    if (data.length <= MAX_BASE64 || quality < 0.4) {
      bitmap.close();
      return { data, url };
    }
  }
}
