// Per-piece audio recorder: one take per piece, saved through the
// piece recordings endpoint. Also wires the share-link copy buttons.
(function () {
  function fmt(s) {
    return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0");
  }

  document.querySelectorAll("[data-copy]").forEach(function (btn) {
    btn.addEventListener("click", function () {
      var url = window.location.origin + btn.getAttribute("data-copy");
      if (navigator.clipboard) {
        navigator.clipboard.writeText(url).then(function () {
          var old = btn.textContent;
          btn.textContent = "✓";
          setTimeout(function () {
            btn.textContent = old;
          }, 1500);
        });
      }
    });
  });

  document.querySelectorAll("[data-recorder]").forEach(function (root) {
    var startBtn = root.querySelector("[data-rec-start]");
    var stopBtn = root.querySelector("[data-rec-stop]");
    var timeEl = root.querySelector("[data-rec-time]");
    var statusEl = root.querySelector("[data-rec-status]");
    var preview = root.querySelector("[data-rec-preview]");
    var actions = root.querySelector("[data-rec-actions]");
    var unsupported = root.querySelector("[data-rec-unsupported]");
    if (!window.MediaRecorder || !navigator.mediaDevices) {
      unsupported.classList.remove("hidden");
      startBtn.disabled = true;
      return;
    }
    var stream = null;
    var recorder = null;
    var chunks = [];
    var blob = null;
    var startedAt = 0;
    var duration = 0;
    var timer = null;

    function reset() {
      blob = null;
      preview.removeAttribute("src");
      preview.classList.add("hidden");
      actions.classList.add("hidden");
      timeEl.textContent = "0:00";
      statusEl.textContent = "";
      startBtn.disabled = false;
      startBtn.classList.remove("hidden");
      stopBtn.disabled = true;
      stopBtn.classList.add("hidden");
    }

    startBtn.addEventListener("click", function () {
      navigator.mediaDevices
        .getUserMedia({ audio: true })
        .then(function (s) {
          stream = s;
          chunks = [];
          recorder = new MediaRecorder(stream);
          recorder.ondataavailable = function (e) {
            if (e.data && e.data.size) chunks.push(e.data);
          };
          recorder.onstop = function () {
            duration = Math.max(1, Math.round((Date.now() - startedAt) / 1000));
            blob = new Blob(chunks, {
              type: recorder.mimeType || "audio/webm",
            });
            preview.src = URL.createObjectURL(blob);
            preview.classList.remove("hidden");
            actions.classList.remove("hidden");
            if (stream)
              stream.getTracks().forEach(function (t) {
                t.stop();
              });
            clearInterval(timer);
            startBtn.disabled = false;
            startBtn.classList.remove("hidden");
            stopBtn.disabled = true;
            stopBtn.classList.add("hidden");
          };
          recorder.start(250);
          startedAt = Date.now();
          timeEl.textContent = "0:00";
          timer = setInterval(function () {
            timeEl.textContent = fmt(
              Math.floor((Date.now() - startedAt) / 1000),
            );
          }, 500);
          startBtn.disabled = true;
          startBtn.classList.add("hidden");
          stopBtn.disabled = false;
          stopBtn.classList.remove("hidden");
        })
        .catch(function () {
          unsupported.classList.remove("hidden");
        });
    });

    stopBtn.addEventListener("click", function () {
      if (recorder && recorder.state !== "inactive") recorder.stop();
    });

    root.querySelector("[data-rec-discard]").addEventListener("click", reset);

    root
      .querySelector("[data-rec-save]")
      .addEventListener("click", function () {
        if (!blob) return;
        var fd = new FormData();
        fd.append("audio", blob, "take");
        fd.append("taken_on", root.getAttribute("data-date"));
      if (root.getAttribute("data-mock-item")) {
        fd.append("mock_item_id", root.getAttribute("data-mock-item"));
      }
        fd.append("duration_sec", String(duration));
        saveBtn.disabled = true;
        statusEl.textContent = "…";
        fetch("/pieces/" + root.getAttribute("data-piece") + "/recordings", {
          method: "POST",
          body: fd,
        })
          .then(function (resp) {
            if (resp.ok) {
              window.location.reload();
            } else {
              statusEl.textContent = "✗";
              saveBtn.disabled = false;
            }
          })
          .catch(function () {
            statusEl.textContent = "✗";
            saveBtn.disabled = false;
          });
      });
    var saveBtn = root.querySelector("[data-rec-save]");
  });
})();
