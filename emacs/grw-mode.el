;;; grw-mode.el --- Minor mode for visualizing grw search patterns -*- lexical-binding: t; -*-

;; Version: 0.1.0
;; Package-Requires: ((emacs "27.1"))
;; Keywords: languages, tools

;;; Commentary:

;; Minor mode for Rust buffers that extracts `search![...]` macro
;; invocations and pipes them through `grw-dot` to produce DOT
;; graph descriptions.  The DOT output is displayed in a side buffer
;; with `graphviz-dot-mode' (if available) for further rendering.

;;; Code:

(defgroup grw nil
  "GRW search pattern visualization."
  :group 'tools
  :prefix "grw-")

(defcustom grw-dot-executable "grw-dot"
  "Path to the grw-dot binary."
  :type 'string
  :group 'grw)

(defcustom grw-dot-program "dot"
  "Path to the graphviz dot program."
  :type 'string
  :group 'grw)

(defun grw--extract-search-at-point ()
  "Extract the inner content of `search![...]' surrounding point.
Returns the string between the brackets, or nil if point is not
inside a search! macro.  Point can be anywhere from the `s' of
`search!' through the closing `]'."
  (save-excursion
    (let ((orig (point))
          start end match-start)
      ;; Move past current word so re-search-backward can match
      ;; search![ when cursor is ON the keyword itself.
      (skip-chars-forward "a-z!\\[")
      (when (re-search-backward "search!\\[" nil t)
        (setq match-start (match-beginning 0))
        (goto-char (match-end 0))
        (setq start (point))
        ;; Go back to the opening bracket
        (backward-char 1)
        ;; Use forward-sexp to find matching ]
        (condition-case nil
            (progn
              (forward-sexp 1)
              (setq end (1- (point)))
              ;; Verify orig was between search! start and closing ]
              (when (and (<= match-start orig) (<= orig (1+ end)))
                (buffer-substring-no-properties start end)))
          (scan-error nil))))))

(defun grw--run-dot (input)
  "Run grw-dot on INPUT string, return DOT output or signal error."
  (with-temp-buffer
    (insert input)
    (let ((exit-code (call-process-region
                      (point-min) (point-max)
                      grw-dot-executable
                      t t nil)))
      (if (zerop exit-code)
          (buffer-string)
        (error "grw-dot failed: %s" (string-trim (buffer-string)))))))

(defun grw-preview ()
  "Extract search! macro at point and display DOT in a side buffer."
  (interactive)
  (let ((content (grw--extract-search-at-point)))
    (unless content
      (user-error "No search![...] macro found at point"))
    (let ((dot (grw--run-dot content))
          (buf (get-buffer-create "*grw-pattern*")))
      (with-current-buffer buf
        (let ((inhibit-read-only t))
          (erase-buffer)
          (insert dot)
          (goto-char (point-min))
          (when (fboundp 'graphviz-dot-mode)
            (graphviz-dot-mode))))
      (display-buffer buf '(display-buffer-reuse-window
                            (reusable-frames . visible))))))

(defun grw-preview-png ()
  "Extract search! macro at point, render to PNG, and display image."
  (interactive)
  (let ((content (grw--extract-search-at-point)))
    (unless content
      (user-error "No search![...] macro found at point"))
    (let* ((dot (grw--run-dot content))
           (png-file (make-temp-file "grw-" nil ".png")))
      (with-temp-buffer
        (insert dot)
        (let ((exit-code (call-process-region
                          (point-min) (point-max)
                          grw-dot-program
                          t nil nil
                          "-Tpng" "-o" png-file)))
          (unless (zerop exit-code)
            (error "dot failed to render PNG"))))
      (let ((buf (get-buffer-create "*grw-pattern-image*")))
        (with-current-buffer buf
          (let ((inhibit-read-only t))
            (erase-buffer)
            (insert-image (create-image png-file 'png nil
                                        :max-width (window-pixel-width)
                                        :max-height (window-pixel-height)))
            (image-mode)))
        (display-buffer buf '(display-buffer-reuse-window
                              (reusable-frames . visible)))))))

;; -=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-
;; Scratch workflow

(defcustom grw-project-root "~/Dev/grw"
  "Root directory of the grw project."
  :type 'directory
  :group 'grw)

(defvar grw--scratch-source-path nil
  "The scratch/<label>.rs path that was opened into bin/scratch.rs.
Buffer-local in the scratch buffer.")

(defvar grw--scratch-template
  "#![allow(unused_imports)]
use grw::*;
use grw::search::{engine::Match, error};

// graph types: graph::Undir0, graph::Dir0, graph::Anydir0
// edge defs:   edge::undir::E::U(a,b)  edge::dir::E::D(a,b)  edge::anydir::E::{U,D}(a,b)
// operators:   ^ (undirected)  >> (directed src->tgt)  << (directed tgt<-src)
// negation:    N(0) & !E() ^ N(1)
// predicates:  N(0).val(v).test(|v| bool)

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
")

(defun grw--scratch-bin ()
  "Return absolute path to bin/scratch.rs."
  (expand-file-name "bin/scratch.rs" grw-project-root))

(defun grw--scratch-dir ()
  "Return absolute path to scratch/."
  (expand-file-name "scratch" grw-project-root))

(defun grw--scratch-label-to-path (label)
  "Convert LABEL like \"cluster/foo\" to scratch/cluster/foo.rs."
  (expand-file-name (concat label ".rs") (grw--scratch-dir)))

(defun grw--read-scratch-label (prompt)
  "Read a scratch label with completion from existing files."
  (let* ((dir (grw--scratch-dir))
         (files (when (file-directory-p dir)
                  (directory-files-recursively dir "\\.rs$")))
         (labels (mapcar (lambda (f)
                           (file-name-sans-extension
                            (file-relative-name f dir)))
                         files)))
    (completing-read prompt labels nil nil)))

(defun grw--scratch-setup-buffer (label)
  "Configure current buffer as scratch with LABEL.
Sets compile command, LSP features, and save-back hook."
  (setq-local grw--scratch-source-path
              (grw--scratch-label-to-path label))
  (setq-local compile-command
              (format "cd %s && cargo run --features scratch --bin scratch -- '%s' 2>&1"
                      (shell-quote-argument (expand-file-name grw-project-root))
                      label))
  (local-set-key (kbd "C-c C-c C-c")
                 (lambda () (interactive) (compile compile-command)))
  (add-hook 'after-save-hook #'grw--scratch-save-back nil t)
  (add-hook 'kill-buffer-hook #'grw--scratch-save-back nil t)
  (when (fboundp 'lsp)
    (setq-local lsp-rust-analyzer-cargo-watch-args ["--features" "scratch"])
    (setq-local lsp-rust-analyzer-cargo-extra-args ["--features" "scratch"])
    (lsp)))

(defun grw--scratch-save-back ()
  "Copy bin/scratch.rs back to its scratch/<label>.rs source."
  (when (and grw--scratch-source-path
             (file-exists-p (grw--scratch-bin)))
    (let ((dir (file-name-directory grw--scratch-source-path)))
      (unless (file-directory-p dir)
        (make-directory dir t)))
    (copy-file (grw--scratch-bin) grw--scratch-source-path t)
    (message "Saved back to %s"
             (file-relative-name grw--scratch-source-path
                                 (expand-file-name grw-project-root)))))

(defun grw-scratch-open (label)
  "Open scratch snippet LABEL into bin/scratch.rs with full LSP."
  (interactive (list (grw--read-scratch-label "Open scratch: ")))
  (let ((src (grw--scratch-label-to-path label))
        (bin (grw--scratch-bin)))
    (unless (file-exists-p src)
      (user-error "No such snippet: %s" src))
    (copy-file src bin t)
    (find-file bin)
    (revert-buffer t t)
    (grw--scratch-setup-buffer label)
    (message "[grw] scratch: %s  |  C-c C-c C-c to run" label)))

(defun grw-scratch-new (label)
  "Create new scratch snippet LABEL and open it."
  (interactive (list (grw--read-scratch-label "New scratch: ")))
  (let ((src (grw--scratch-label-to-path label))
        (bin (grw--scratch-bin)))
    (when (file-exists-p src)
      (unless (y-or-n-p (format "%s exists. Open anyway?" label))
        (user-error "Aborted")))
    (unless (file-exists-p src)
      (let ((dir (file-name-directory src)))
        (unless (file-directory-p dir)
          (make-directory dir t)))
      (with-temp-file src
        (insert grw--scratch-template)))
    (copy-file src bin t)
    (find-file bin)
    (revert-buffer t t)
    (grw--scratch-setup-buffer label)
    (message "[grw] new scratch: %s  |  C-c C-c C-c to run" label)))

;; -=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-
;; Mode definition

(defvar grw-mode-map
  (let ((map (make-sparse-keymap)))
    (define-key map (kbd "C-c C-s p") #'grw-preview-png)
    (define-key map (kbd "C-c C-s d") #'grw-preview)
    (define-key map (kbd "C-c C-s o") #'grw-scratch-open)
    (define-key map (kbd "C-c C-s n") #'grw-scratch-new)
    map)
  "Keymap for `grw-mode'.")

;;;###autoload
(define-minor-mode grw-mode
  "Minor mode for visualizing grw search! patterns.

\\{grw-mode-map}"
  :lighter " GRW"
  :keymap grw-mode-map)

(provide 'grw-mode)

;;; grw-mode.el ends here
