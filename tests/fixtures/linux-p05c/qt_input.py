"""P05C 手动输入验收窗口。只使用合成文本。"""

import sys

from PyQt6.QtCore import Qt
from PyQt6.QtWidgets import QApplication, QFormLayout, QLabel, QLineEdit, QWidget


def main() -> int:
    app = QApplication(sys.argv)
    window = QWidget()
    window.setWindowTitle("P05C · Qt 输入验收")
    layout = QFormLayout(window)
    layout.addRow(QLabel("仅输入合成文本。用 Tab 往返两个字段，观察候选与提交。"))

    ordinary = QLineEdit()
    sensitive = QLineEdit()
    sensitive.setInputMethodHints(Qt.InputMethodHint.ImhSensitiveData)
    layout.addRow("普通文本", ordinary)
    layout.addRow("敏感文本（明文显示）", sensitive)
    QWidget.setTabOrder(ordinary, sensitive)
    ordinary.setFocus()

    window.show()
    return app.exec()


if __name__ == "__main__":
    sys.exit(main())
