import ApplicationServices
import AppKit

// Надиктованное — сразу в поле, где стоит курсор (ТЗ 01.11.2026: «чтобы буфер обмена сразу вставлял в выбранное
// поле; если поле не выбрано — не вставляет»). Нужен «Универсальный доступ»: спросить, что в фокусе у другой
// программы, и нажать за человека ⌘V. Текст при этом остаётся и в буфере обмена.
enum AutoPaste {
    static var enabled: Bool {
        get { UserDefaults.standard.bool(forKey: "autoPaste") }
        set { UserDefaults.standard.set(newValue, forKey: "autoPaste"); if newValue { askAccess() } }
    }

    static var trusted: Bool { AXIsProcessTrusted() }

    // Системное окно «разрешить универсальный доступ»; дальше человек включает сам.
    static func askAccess() {
        _ = AXIsProcessTrustedWithOptions([kAXTrustedCheckOptionPrompt.takeUnretainedValue(): true] as CFDictionary)
    }

    // Вставляет, только если в фокусе поле, куда можно писать. true — вставили.
    @discardableResult
    static func intoFocusedField() -> Bool {
        guard enabled, trusted, focusedFieldIsEditable() else { return false }
        // Буфер только что записан, а хоткей ещё может держать клавиши — ⌘V чуть позже, иначе вставка теряется.
        usleep(150_000)
        let source = CGEventSource(stateID: .combinedSessionState)
        let v: CGKeyCode = 9
        for down in [true, false] {
            let event = CGEvent(keyboardEventSource: source, virtualKey: v, keyDown: down)
            event?.flags = .maskCommand
            event?.post(tap: .cghidEventTap)
        }
        return true
    }

    // Поле — то, у чего есть курсор (выделенный диапазон текста), значение, которое можно менять, или роль
    // текстового поля. Спрашиваем у программы впереди, а не у системы целиком: Chrome, Telegram и другие на
    // Electron отвечают только так, и только после AXManualAccessibility (03.11.2026, «не работает вставлять»).
    static func focusedFieldIsEditable() -> Bool {
        guard let app = NSWorkspace.shared.frontmostApplication, app.processIdentifier != getpid() else { return false }
        let element = AXUIElementCreateApplication(app.processIdentifier)
        AXUIElementSetAttributeValue(element, "AXManualAccessibility" as CFString, kCFBooleanTrue)
        var focused: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, kAXFocusedUIElementAttribute as CFString, &focused) == .success,
              let focused, CFGetTypeID(focused) == AXUIElementGetTypeID() else { return false }
        let field = focused as! AXUIElement
        var role: CFTypeRef?
        AXUIElementCopyAttributeValue(field, kAXRoleAttribute as CFString, &role)
        if let role = role as? String, [kAXTextFieldRole, kAXTextAreaRole, kAXComboBoxRole, "AXSearchField"].contains(role) { return true }
        var settable: DarwinBoolean = false
        if AXUIElementIsAttributeSettable(field, kAXValueAttribute as CFString, &settable) == .success, settable.boolValue { return true }
        var range: CFTypeRef?
        return AXUIElementCopyAttributeValue(field, kAXSelectedTextRangeAttribute as CFString, &range) == .success
    }
}
