export function isTextEditingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) {
    return false;
  }

  if (target.closest('.cm-editor')) {
    return true;
  }

  const editable = target.closest(
    'input, textarea, select, [contenteditable=""], [contenteditable="true"]',
  );
  if (!editable) {
    return false;
  }

  if (
    editable instanceof HTMLInputElement &&
    [
      'button',
      'checkbox',
      'color',
      'file',
      'image',
      'radio',
      'range',
      'reset',
      'submit',
    ].includes(editable.type)
  ) {
    return false;
  }

  return true;
}

export function shouldIgnoreEscapeClose(target: EventTarget | null): boolean {
  return isTextEditingTarget(target);
}
