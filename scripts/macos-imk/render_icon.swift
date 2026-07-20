import AppKit
import Foundation

guard CommandLine.arguments.count == 3 else {
    fputs("usage: render_icon.swift <input-svg> <output-tiff>\n", stderr)
    exit(2)
}

let input = CommandLine.arguments[1]
let output = CommandLine.arguments[2]
guard let image = NSImage(contentsOfFile: input) else {
    fputs("unable to load input icon\n", stderr)
    exit(1)
}
let logicalSize = 16
let pixelSize = 32
let artworkInset = 1
guard let bitmap = NSBitmapImageRep(
    bitmapDataPlanes: nil,
    pixelsWide: pixelSize,
    pixelsHigh: pixelSize,
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
bitmap.size = NSSize(width: logicalSize, height: logicalSize)

NSGraphicsContext.saveGraphicsState()
guard let graphicsContext = NSGraphicsContext(bitmapImageRep: bitmap) else {
    fputs("unable to create icon graphics context\n", stderr)
    exit(1)
}
NSGraphicsContext.current = graphicsContext
NSColor.clear.setFill()
NSRect(x: 0, y: 0, width: logicalSize, height: logicalSize).fill()
image.draw(
    in: NSRect(
        x: artworkInset,
        y: artworkInset,
        width: logicalSize - artworkInset * 2,
        height: logicalSize - artworkInset * 2
    ),
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
