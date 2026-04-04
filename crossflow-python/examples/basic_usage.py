from crossflow_python import Executor


def main():
    executor = Executor()
    executor.register_node(
        "add_fee",
        lambda request, config: {
            **request,
            "total": round(request["subtotal"] + config["fee"], 2),
        },
        description="Add a flat fee to an order subtotal",
    )
    executor.register_node(
        "format_summary",
        lambda request, config: (
            f"{config['label']}: {request['customer']} owes ${request['total']:.2f}"
        ),
        description="Format the final total as a short message",
    )

    diagram = {
        "version": "0.1.0",
        "start": "fee_step",
        "ops": {
            "fee_step": {
                "type": "node",
                "builder": "add_fee",
                "config": {"fee": 2.5},
                "next": "summary_step",
            },
            "summary_step": {
                "type": "node",
                "builder": "format_summary",
                "config": {"label": "Invoice"},
                "next": {"builtin": "terminate"},
            },
        },
    }

    request = {"customer": "Ada", "subtotal": 17.5}
    metadata = executor.metadata()
    result = executor.run(diagram, request)

    print("registered nodes:", ", ".join(sorted(metadata["nodes"].keys())))
    print("request:", request)
    print("flow: add_fee -> format_summary")
    print("result:", result)


if __name__ == "__main__":
    main()
