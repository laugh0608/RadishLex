import AppKit
import Foundation

guard CommandLine.arguments.count == 3 else {
    fputs("usage: render_icon.swift <input-svg> <output-png>\n", stderr)
    exit(2)
}

let input = CommandLine.arguments[1]
let output = CommandLine.arguments[2]
guard let image = NSImage(contentsOfFile: input) else {
    fputs("unable to load input icon\n", stderr)
    exit(1)
}
guard let bitmap = NSBitmapImageRep(
    bitmapDataPlanes: nil,
    pixelsWide: 64,
    pixelsHigh: 64,
    bitsPerSample: 8,
    samplesPerPixel: 4,
    hasAlpha: true,
    isPlanar: false,
    colorSpaceName: .deviceRGB,
    bytesPerRow: 0,
    bitsPerPixel: 0
) else {
    fputs("unable to allocate icon bitmap\n", stderr)
    exit(1)
}

NSGraphicsContext.saveGraphicsState()
guard let graphicsContext = NSGraphicsContext(bitmapImageRep: bitmap) else {
    fputs("unable to create icon graphics context\n", stderr)
    exit(1)
}
NSGraphicsContext.current = graphicsContext
NSColor.clear.setFill()
NSRect(x: 0, y: 0, width: 64, height: 64).fill()
image.draw(
    in: NSRect(x: 0, y: 0, width: 64, height: 64),
    from: .zero,
    operation: .sourceOver,
    fraction: 1
)
graphicsContext.flushGraphics()
NSGraphicsContext.restoreGraphicsState()

guard let tiff = bitmap.representation(using: .tiff, properties: [:]) else {
    fputs("unable to encode icon TIFF\n", stderr)
    exit(1)
}
try tiff.write(to: URL(fileURLWithPath: output), options: .atomic)
