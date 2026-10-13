export default {
  extends: ["stylelint-config-standard"],
  rules: {
    "no-descending-specificity": null,
    "property-no-vendor-prefix": [
      true,
      {
        ignoreProperties: [
          "-webkit-backdrop-filter",
          "-webkit-appearance",
          "backdrop-filter",
          "appearance",
        ],
      },
    ],
  },
};
