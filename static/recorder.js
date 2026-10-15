// Per-piece audio recorder: one take per piece, saved through the
// piece recordings endpoint. Also wires the share-link copy buttons.
(() => {
  function fmt(s) {
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  }

  document.querySelectorAll("[data-copy]").forEach((btn) => {
    btn.addEventListener("click", () => {
      var url = window.location.origin + btn.getAttribute("data-copy");
      if (navigator.clipboard) {
        navigator.clipboard.writeText(url).then(() => {
          var old = btn.textContent;
          btn.textContent = "✓";
          setTimeout(() => {
            btn.textContent = old;
          }, 1500);
        });
      }
    });
  });

  document.querySelectorAll("[data-recorder]").forEach((root) => {
    var startBtn = root.querySelector("[data-rec-start]");
    var stopBtn = root.querySelector("[data-rec-stop]");
    var timeEl = root.querySelector("[data-rec-time]");
    var statusEl = root.querySelector("[data-rec-status]");
    var preview = root.querySelector("[data-rec-preview]");
    var actions = root.querySelector("[data-rec-actions]");
    var unsupported = root.querySelector("[data-rec-unsupported]");
    var meter = root.querySelector("[data-rec-meter]");
    var meterFill = root.querySelector("[data-rec-meter-fill]");
    if (!window.MediaRecorder || !navigator.mediaDevices) {
      unsupported.classList.remove("hidden");
      startBtn.disabled = true;
      return;
    }
    var stream = null;
    var recorder = null;
    var chunks = [];
    var blob = null;
    var previewURL = null;
    var startedAt = 0;
    var duration = 0;
    var timer = null;
    var audioCtx = null;
    var meterTimer = null;

    function startMeter() {
      var AC = window.AudioContext || window.webkitAudioContext;
      if (!AC || !stream || !meter) return;
      var analyser;
      var data;
      try {
        audioCtx = new AC();
        if (audioCtx.state === "suspended") audioCtx.resume();
        analyser = audioCtx.createAnalyser();
        analyser.fftSize = 2048;
        audioCtx.createMediaStreamSource(stream).connect(analyser);
        data = new Uint8Array(analyser.fftSize);
        meter.classList.remove("hidden");
        meterTimer = setInterval(() => {
          analyser.getByteTimeDomainData(data);
          var peak = 0;
          var i;
          var v;
          for (i = 0; i < data.length; i++) {
            v = Math.abs(data[i] - 128) / 128;
            if (v > peak) peak = v;
          }
          meterFill.style.width = `${Math.min(100, Math.round(peak * 100))}%`;
        }, 100);
      } catch {
        stopMeter();
      }
    }

    function stopMeter() {
      if (meterTimer) clearInterval(meterTimer);
      meterTimer = null;
      if (audioCtx) audioCtx.close();
      audioCtx = null;
      if (meterFill) meterFill.style.width = "0%";
      if (meter) meter.classList.add("hidden");
    }

    function reset() {
      stopMeter();
      blob = null;
      preview.pause();
      if (previewURL) URL.revokeObjectURL(previewURL);
      previewURL = null;
      preview.removeAttribute("src");
      preview.load();
      preview.classList.add("hidden");
      actions.classList.add("hidden");
      timeEl.textContent = "0:00";
      statusEl.textContent = "";
      startBtn.disabled = false;
      startBtn.classList.remove("hidden");
      stopBtn.disabled = true;
      stopBtn.classList.add("hidden");
    }

    startBtn.addEventListener("click", () => {
      navigator.mediaDevices
        .getUserMedia({ audio: true })
        .then((s) => {
          stream = s;
          chunks = [];
          recorder = new MediaRecorder(stream);
          recorder.ondataavailable = (e) => {
            if (e.data?.size) chunks.push(e.data);
          };
          recorder.onstop = () => {
            duration = Math.max(1, Math.round((Date.now() - startedAt) / 1000));
            blob = new Blob(chunks, {
              type: recorder.mimeType || "audio/webm",
            });
            previewURL = URL.createObjectURL(blob);
            preview.src = previewURL;
            preview.classList.remove("hidden");
            actions.classList.remove("hidden");
            if (stream)
              stream.getTracks().forEach((t) => {
                t.stop();
              });
            clearInterval(timer);
            stopMeter();
            startBtn.disabled = false;
            startBtn.classList.remove("hidden");
            stopBtn.disabled = true;
            stopBtn.classList.add("hidden");
          };
          recorder.start(250);
          startMeter();
          startedAt = Date.now();
          timeEl.textContent = "0:00";
          timer = setInterval(() => {
            timeEl.textContent = fmt(
              Math.floor((Date.now() - startedAt) / 1000),
            );
          }, 500);
          startBtn.disabled = true;
          startBtn.classList.add("hidden");
          stopBtn.disabled = false;
          stopBtn.classList.remove("hidden");
        })
        .catch(() => {
          unsupported.classList.remove("hidden");
        });
    });

    stopBtn.addEventListener("click", () => {
      if (recorder && recorder.state !== "inactive") recorder.stop();
    });

    root.querySelector("[data-rec-discard]").addEventListener("click", reset);

    root.querySelector("[data-rec-save]").addEventListener("click", () => {
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
      fetch(`/pieces/${root.getAttribute("data-piece")}/recordings`, {
        method: "POST",
        body: fd,
      })
        .then((resp) => {
          if (resp.ok) {
            window.location.reload();
          } else {
            statusEl.textContent = "✗";
            saveBtn.disabled = false;
          }
        })
        .catch(() => {
          statusEl.textContent = "✗";
          saveBtn.disabled = false;
        });
    });
    var saveBtn = root.querySelector("[data-rec-save]");
  });
})();
